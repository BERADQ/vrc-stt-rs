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

/// Maximum input samples for 10ms at 48kHz
const MAX_INPUT_SAMPLES: usize = 480;

/// Resampler that converts input audio to 16kHz mono
pub struct AudioResampler {
    resampler: Option<Async<f32>>,
    input_channels: usize,
    /// Pre-allocated buffer for mixed mono audio
    input_buffer: Vec<f32>,
    /// Channel buffer for resampler
    channel_buffer: Vec<Vec<f32>>,
}

impl AudioResampler {
    /// Create a new resampler
    ///
    /// # Arguments
    /// * `input_sample_rate` - The sample rate of the input audio
    /// * `input_channels` - Number of input channels (will be mixed to mono)
    ///
    /// Returns `Ok(None)` if no resampling is needed (already 16kHz mono)
    pub fn new(
        input_sample_rate: u32,
        input_channels: usize,
    ) -> anyhow::Result<Option<Self>> {
        // No processing needed for 16kHz mono
        if input_sample_rate == TARGET_SAMPLE_RATE && input_channels == 1 {
            return Ok(None);
        }

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        // Calculate chunk size: 10ms at input sample rate
        let chunk_size = (input_sample_rate as usize * 10) / 1000;

        let resampler = Async::new_sinc(
            TARGET_SAMPLE_RATE as f64 / input_sample_rate as f64,
            1.0,
            &params,
            chunk_size,
            1, // output channels: always mono
            FixedAsync::Input,
        )?;

        Ok(Some(Self {
            resampler: Some(resampler),
            input_channels,
            input_buffer: Vec::with_capacity(MAX_INPUT_SAMPLES),
            channel_buffer: vec![Vec::with_capacity(MAX_INPUT_SAMPLES); input_channels.max(1)],
        }))
    }

    /// Mix interleaved multi-channel audio to mono
    fn mix_to_mono(&self, input: &[f32]) -> Vec<f32> {
        if self.input_channels == 1 {
            return input.to_vec();
        }

        let frames = input.len() / self.input_channels;
        let mut mono = Vec::with_capacity(frames);

        for frame_idx in 0..frames {
            let mut sum = 0.0f32;
            for ch in 0..self.input_channels {
                sum += input[frame_idx * self.input_channels + ch];
            }
            mono.push(sum / self.input_channels as f32);
        }

        mono
    }

    /// Resample audio data to 16kHz mono
    ///
    /// Returns resampled data in an ArrayVec to avoid heap allocation.
    /// The output is always exactly `CHUNK_SIZE` samples (160 for 16kHz 10ms).
    pub fn resample(&mut self, input: &[f32]) -> anyhow::Result<ArrayVec<f32, CHUNK_SIZE>> {
        // First, mix to mono if needed
        let mono_input = if self.input_channels > 1 {
            self.input_buffer.clear();
            self.input_buffer.extend_from_slice(&self.mix_to_mono(input));
            &self.input_buffer
        } else {
            input
        };

        // No resampling needed case
        let Some(resampler) = &mut self.resampler else {
            let mut result = ArrayVec::<f32, CHUNK_SIZE>::new();
            result.try_extend_from_slice(mono_input).map_err(|_| {
                anyhow::anyhow!(
                    "Input too large: {} > {}",
                    mono_input.len(),
                    CHUNK_SIZE
                )
            })?;
            return Ok(result);
        };

        // Update channel buffer
        self.channel_buffer[0].clear();
        self.channel_buffer[0].extend_from_slice(mono_input);

        // Process through resampler
        use audioadapter_buffers::direct::SequentialSliceOfVecs;
        let input_adapter =
            SequentialSliceOfVecs::new(&self.channel_buffer, 1, mono_input.len())?;
        let output = resampler.process(&input_adapter, 0, None)?;

        // Convert to ArrayVec
        let output_vec = output.take_data();
        let mut result = ArrayVec::<f32, CHUNK_SIZE>::new();
        result.try_extend_from_slice(&output_vec).map_err(|_| {
            anyhow::anyhow!(
                "Output too large: {} > {}",
                output_vec.len(),
                CHUNK_SIZE
            )
        })?;

        Ok(result)
    }
}
