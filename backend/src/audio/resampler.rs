//! Audio resampler
//!
//! Converts audio from various sample rates to 16kHz mono using high-quality
//! sinc interpolation.

use arrayvec::ArrayVec;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use super::{CHUNK_SIZE, TARGET_SAMPLE_RATE};
use common::config::ChannelMixMode;

/// Maximum input frames for 10ms at 48kHz stereo
const MAX_INPUT_FRAMES: usize = 480;

/// Resampler that converts input audio to 16kHz mono
pub struct AudioResampler {
    resampler: Option<Async<f32>>,
    input_channels: usize,
    /// Number of frames needed for one resampler call
    input_chunk_frames: usize,
    /// Channel mixing mode
    mix_mode: ChannelMixMode,
    /// Pre-allocated buffer for mixed mono audio (input accumulation)
    mono_buffer: Vec<f32>,
    /// Channel buffer for resampler (planar format)
    channel_buffer: Vec<Vec<f32>>,
}

impl AudioResampler {
    /// Create a new resampler
    ///
    /// # Arguments
    /// * `input_sample_rate` - The sample rate of the input audio
    /// * `input_channels` - Number of input channels (will be mixed to mono)
    /// * `mix_mode` - Channel mixing mode for multi-channel audio
    ///
    /// Returns `Ok(None)` if no resampling is needed (already 16kHz mono)
    pub fn new(
        input_sample_rate: u32,
        input_channels: usize,
        mix_mode: ChannelMixMode,
    ) -> anyhow::Result<Option<Self>> {
        // No processing needed for 16kHz mono
        if input_sample_rate == TARGET_SAMPLE_RATE && input_channels == 1 {
            return Ok(None);
        }

        // Calculate chunk size in frames: 10ms at input sample rate
        let input_chunk_frames = (input_sample_rate as usize * 10) / 1000;

        // Only resampling needed (already mono or need to mix)
        let resampler = if input_sample_rate != TARGET_SAMPLE_RATE {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            };

            Some(Async::new_sinc(
                TARGET_SAMPLE_RATE as f64 / input_sample_rate as f64,
                1.0,
                &params,
                input_chunk_frames,
                1, // output channels: always mono
                FixedAsync::Input,
            )?)
        } else {
            None
        };

        Ok(Some(Self {
            resampler,
            input_channels,
            input_chunk_frames,
            mix_mode,
            mono_buffer: Vec::with_capacity(MAX_INPUT_FRAMES),
            channel_buffer: vec![Vec::with_capacity(MAX_INPUT_FRAMES); 1],
        }))
    }

    /// Process interleaved multi-channel audio to mono
    ///
    /// Supports multiple mixing modes:
    /// - MixToMono: RMS energy-preserving mix to avoid phase cancellation
    /// - FirstChannel: Use only the first (left) channel
    /// - SecondChannel: Use only the second (right) channel
    fn process_channels(
        input: &[f32],
        input_channels: usize,
        mix_mode: ChannelMixMode,
        output: &mut Vec<f32>,
    ) {
        if input_channels == 1 {
            output.extend_from_slice(input);
            return;
        }

        let frames = input.len() / input_channels;
        output.reserve(frames);

        match mix_mode {
            ChannelMixMode::MixToMono => {
                // RMS energy-preserving mix to avoid phase cancellation
                for frame_idx in 0..frames {
                    let base_idx = frame_idx * input_channels;

                    let mut sum_squares = 0.0f32;
                    for ch in 0..input_channels {
                        let sample = input[base_idx + ch];
                        sum_squares += sample * sample;
                    }

                    let rms = (sum_squares / input_channels as f32).sqrt();
                    let dominant = input[base_idx];
                    let mixed = if dominant < 0.0 { -rms } else { rms };

                    output.push(mixed);
                }
            }
            ChannelMixMode::FirstChannel => {
                // Use only first channel
                for frame_idx in 0..frames {
                    output.push(input[frame_idx * input_channels]);
                }
            }
            ChannelMixMode::SecondChannel => {
                // Use only second channel (if available)
                if input_channels >= 2 {
                    for frame_idx in 0..frames {
                        output.push(input[frame_idx * input_channels + 1]);
                    }
                } else {
                    // Fall back to first channel if only one channel
                    for frame_idx in 0..frames {
                        output.push(input[frame_idx * input_channels]);
                    }
                }
            }
        }
    }

    /// Process accumulated mono frames through resampler
    fn process_resampler(&mut self) -> anyhow::Result<ArrayVec<f32, CHUNK_SIZE>> {
        let resampler = self.resampler.as_mut().unwrap();

        // Update channel buffer with accumulated frames
        self.channel_buffer[0].clear();
        self.channel_buffer[0].extend_from_slice(&self.mono_buffer[..self.input_chunk_frames]);

        // Process through resampler
        use audioadapter_buffers::direct::SequentialSliceOfVecs;
        let input_adapter =
            SequentialSliceOfVecs::new(&self.channel_buffer, 1, self.input_chunk_frames)?;
        let output = resampler.process(&input_adapter, 0, None)?;

        // Remove processed frames from buffer
        self.mono_buffer.drain(..self.input_chunk_frames);

        // Convert to ArrayVec
        let output_vec = output.take_data();
        let mut result = ArrayVec::<f32, CHUNK_SIZE>::new();
        result.try_extend_from_slice(&output_vec).map_err(|_| {
            anyhow::anyhow!("Output too large: {} > {}", output_vec.len(), CHUNK_SIZE)
        })?;

        Ok(result)
    }

    /// Resample audio data to 16kHz mono
    ///
    /// Returns resampled data in an ArrayVec to avoid heap allocation.
    /// The output is always exactly `CHUNK_SIZE` samples (160 for 16kHz 10ms).
    /// Note: This method may return an empty ArrayVec if not enough input data
    /// has been accumulated yet.
    pub fn resample(&mut self, input: &[f32]) -> anyhow::Result<ArrayVec<f32, CHUNK_SIZE>> {
        // Process input channels and add to buffer
        let input_channels = self.input_channels;
        let mix_mode = self.mix_mode;
        Self::process_channels(input, input_channels, mix_mode, &mut self.mono_buffer);

        // Check if we have enough frames for processing
        if self.mono_buffer.len() < self.input_chunk_frames {
            // Not enough data yet, return empty
            return Ok(ArrayVec::new());
        }

        // No resampling needed case (just channel mixing)
        if self.resampler.is_none() {
            let frames_to_take = self.input_chunk_frames.min(self.mono_buffer.len());
            let mut result = ArrayVec::<f32, CHUNK_SIZE>::new();
            result
                .try_extend_from_slice(&self.mono_buffer[..frames_to_take])
                .map_err(|_| {
                    anyhow::anyhow!("Buffer size mismatch: {} > {}", frames_to_take, CHUNK_SIZE)
                })?;
            self.mono_buffer.drain(..frames_to_take);
            return Ok(result);
        }

        // Process through resampler
        self.process_resampler()
    }
}
