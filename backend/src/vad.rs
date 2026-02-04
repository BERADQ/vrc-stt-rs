use common::config::ConfigManager;

pub struct VadWrapper {
    vad: webrtc_vad::Vad,
    debounce_count: usize,
    threshold: f32,
    in_voice: bool,
    buffer: Vec<f32>,
    config_manager: ConfigManager,
}

impl VadWrapper {
    pub fn new(config_manager: &ConfigManager) -> Self {
        let config = config_manager.config();
        Self {
            vad: webrtc_vad::Vad::new_with_rate_and_mode(
                webrtc_vad::SampleRate::Rate16kHz,
                webrtc_vad::VadMode::VeryAggressive,
            ),
            threshold: (config.vad.threshold_level.abs() * 1.0).round(),
            debounce_count: 0,
            in_voice: false,
            buffer: Vec::new(),
            config_manager: config_manager.clone(),
        }
    }

    /// Returns debounced voice state for the given audio chunk.
    fn is_voice_segment(&mut self, data: &[f32]) -> Result<bool, ()> {
        // First check VAD
        let mut data_i16: [i16; 160] = [0; 160];
        f32_samples_to_i16(data, &mut data_i16);
        let is_voice_vad = self.vad.is_voice_segment(&data_i16)?;

        // VAD says voice, check amplitude threshold
        let threshold = self.threshold;
        let exceeds_threshold = data.iter().any(|&sample| sample.abs() > threshold);

        return Ok(self.debounce(is_voice_vad && exceeds_threshold));
    }

    fn debounce(&mut self, is_voice: bool) -> bool {
        if !is_voice {
            // VAD says not voice, apply debounce
            let config = self.config_manager.config();
            if self.debounce_count >= config.vad.debounce_times {
                return false;
            }
            self.debounce_count += 1;
            return true;
        }

        // VAD says voice, reset debounce counter
        self.debounce_count = 0;
        true
    }

    pub fn segment_parse(&mut self, data: &[f32]) -> Result<VadEvent, ()> {
        let current_voice = self.is_voice_segment(data)?;
        let prev_voice = self.in_voice;

        match (prev_voice, current_voice) {
            (false, true) => {
                // Voice just started
                self.in_voice = true;
                self.buffer.clear();
                // Convert i16 samples to f32 in range [-1.0, 1.0]
                self.buffer.extend(data);
                Ok(VadEvent::Start)
            }
            (true, true) => {
                // Voice continues, accumulate samples
                self.buffer.extend(data);
                Ok(VadEvent::Recording)
            }
            (true, false) => {
                // Voice ended after debounce
                self.in_voice = false;
                let voice_data = std::mem::take(&mut self.buffer);
                Ok(VadEvent::End(voice_data))
            }
            (false, false) => {
                // Silence continues
                Ok(VadEvent::Pending)
            }
        }
    }
}

unsafe impl Send for VadWrapper {}

pub enum VadEvent {
    Start,
    Pending,
    Recording,
    End(Vec<f32>),
}

fn f32_samples_to_i16(samples: &[f32], buffer: &mut [i16]) {
    for (sample, out) in samples.iter().zip(buffer.iter_mut()) {
        *out = (*sample * 32768.0).round() as i16;
    }
}
