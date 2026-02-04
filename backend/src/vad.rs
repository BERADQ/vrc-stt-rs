//! Voice Activity Detection (VAD) wrapper
//!
//! Wraps WebRTC VAD with amplitude-based thresholding and debounce logic.

use common::config::{ConfigManager, VadMode};

/// VAD wrapper with threshold and debounce
pub struct VadWrapper {
    vad: webrtc_vad::Vad,
    debounce_count: usize,
    threshold: f32,
    in_voice: bool,
    buffer: Vec<f32>,
    config_manager: ConfigManager,
}

impl VadWrapper {
    /// Create a new VAD wrapper
    pub fn new(config_manager: &ConfigManager) -> Self {
        let config = config_manager.config();
        Self {
            vad: webrtc_vad::Vad::new_with_rate_and_mode(
                webrtc_vad::SampleRate::Rate16kHz,
                vad_mode_to_webrtc(config.vad.mode),
            ),
            threshold: config.vad.threshold_level.abs(),
            debounce_count: 0,
            in_voice: false,
            buffer: Vec::new(),
            config_manager: config_manager.clone(),
        }
    }

    /// Check if the audio chunk contains voice
    ///
    /// Applies both VAD and amplitude threshold
    fn is_voice_segment(&mut self, data: &[f32]) -> bool {
        // Check VAD
        let mut data_i16 = [0i16; 160];
        f32_to_i16_samples(data, &mut data_i16);

        let is_voice_vad = self.vad.is_voice_segment(&data_i16).unwrap_or(false);

        // Check amplitude threshold
        let exceeds_threshold = data.iter().any(|&s| s.abs() > self.threshold);

        self.debounce(is_voice_vad && exceeds_threshold)
    }

    /// Debounce voice detection
    fn debounce(&mut self, is_voice: bool) -> bool {
        if !is_voice {
            let config = self.config_manager.config();
            if self.debounce_count >= config.vad.debounce_times {
                return false;
            }
            self.debounce_count += 1;
            return true;
        }

        self.debounce_count = 0;
        true
    }

    /// Process a segment of audio
    ///
    /// Returns the VAD event: Start, Recording, End, or Pending
    pub fn segment_parse(&mut self, data: &[f32]) -> VadEvent {
        let current_voice = self.is_voice_segment(data);
        let prev_voice = self.in_voice;

        match (prev_voice, current_voice) {
            (false, true) => {
                // Voice started
                self.in_voice = true;
                self.buffer.clear();
                self.buffer.extend_from_slice(data);
                VadEvent::Start
            }
            (true, true) => {
                // Voice continues
                self.buffer.extend_from_slice(data);
                VadEvent::Recording
            }
            (true, false) => {
                // Voice ended
                self.in_voice = false;
                let voice_data = std::mem::take(&mut self.buffer);
                VadEvent::End(voice_data)
            }
            (false, false) => {
                // Silence
                VadEvent::Pending
            }
        }
    }
}

// SAFETY: webrtc_vad::Vad is not Send by default, but it's safe to send
// because we only use it from a single thread (the audio callback thread)
unsafe impl Send for VadWrapper {}

/// VAD events
#[derive(Debug)]
pub enum VadEvent {
    /// Voice activity started
    Start,
    /// No voice detected
    Pending,
    /// Voice continuing
    Recording,
    /// Voice ended with captured audio
    End(Vec<f32>),
}

/// Convert f32 samples (range [-1.0, 1.0]) to i16
fn f32_to_i16_samples(samples: &[f32], output: &mut [i16]) {
    for (sample, out) in samples.iter().zip(output.iter_mut()) {
        *out = (*sample * 32768.0).clamp(-32768.0, 32767.0) as i16;
    }
}

/// Convert our VadMode to webrtc_vad::VadMode
fn vad_mode_to_webrtc(mode: VadMode) -> webrtc_vad::VadMode {
    match mode {
        VadMode::Quality => webrtc_vad::VadMode::Quality,
        VadMode::LowBitrate => webrtc_vad::VadMode::LowBitrate,
        VadMode::Aggressive => webrtc_vad::VadMode::Aggressive,
        VadMode::VeryAggressive => webrtc_vad::VadMode::VeryAggressive,
    }
}
