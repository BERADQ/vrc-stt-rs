use arrayvec::ArrayVec;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

/// Maximum input samples for 10ms at 48kHz (highest supported sample rate)
const MAX_INPUT_SAMPLES: usize = 480; // 48kHz * 10ms
/// Output samples for 10ms at 16kHz
const OUTPUT_SAMPLES: usize = 160; // 16kHz * 10ms

/// Resampler that converts input sample rate to 16kHz
pub struct AudioResampler {
    resampler: Option<Async<f32>>,
    /// Pre-allocated input buffer to avoid heap allocation
    input_buffer: Vec<f32>,
    /// Scratch buffer for converting ArrayVec to slice
    channel_buffer: Vec<Vec<f32>>,
}

impl AudioResampler {
    /// Create a new resampler. If input_sample_rate is already 16000, no resampling is performed.
    pub fn new(input_sample_rate: u32) -> anyhow::Result<Self> {
        const OUTPUT_SAMPLE_RATE: u32 = 16000;

        if input_sample_rate == OUTPUT_SAMPLE_RATE {
            return Ok(Self {
                resampler: None,
                input_buffer: Vec::with_capacity(MAX_INPUT_SAMPLES),
                channel_buffer: vec![Vec::with_capacity(MAX_INPUT_SAMPLES)],
            });
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
            OUTPUT_SAMPLE_RATE as f64 / input_sample_rate as f64,
            1.0,
            &params,
            chunk_size,
            1, // channels
            FixedAsync::Input,
        )?;

        Ok(Self {
            resampler: Some(resampler),
            input_buffer: Vec::with_capacity(MAX_INPUT_SAMPLES),
            channel_buffer: vec![Vec::with_capacity(MAX_INPUT_SAMPLES)],
        })
    }

    /// Resample audio data to 16kHz
    /// Returns resampled data using ArrayVec to avoid heap allocation
    pub fn resample(&mut self, input: &[f32]) -> anyhow::Result<ArrayVec<f32, OUTPUT_SAMPLES>> {
        let Some(resampler) = &mut self.resampler else {
            // No resampling needed, copy to ArrayVec
            let mut result = ArrayVec::<f32, OUTPUT_SAMPLES>::new();
            result.try_extend_from_slice(input).map_err(|_| {
                anyhow::anyhow!("Input too large for output buffer: {} > {}", input.len(), OUTPUT_SAMPLES)
            })?;
            return Ok(result);
        };

        // Copy input to pre-allocated buffer (avoiding allocation)
        self.input_buffer.clear();
        self.input_buffer.extend_from_slice(input);
        
        // Update channel buffer reference
        self.channel_buffer[0].clear();
        self.channel_buffer[0].extend_from_slice(input);

        // Create adapter for input using audioadapter_buffers
        use audioadapter_buffers::direct::SequentialSliceOfVecs;
        let input_adapter = SequentialSliceOfVecs::new(&self.channel_buffer, 1, input.len())?;

        // Process the input
        let output = resampler.process(&input_adapter, 0, None)?;

        // Convert to ArrayVec to avoid heap allocation
        let output_vec = output.take_data();
        let mut result = ArrayVec::<f32, OUTPUT_SAMPLES>::new();
        result.try_extend_from_slice(&output_vec).map_err(|_| {
            anyhow::anyhow!("Output too large: {} > {}", output_vec.len(), OUTPUT_SAMPLES)
        })?;
        
        Ok(result)
    }
}
