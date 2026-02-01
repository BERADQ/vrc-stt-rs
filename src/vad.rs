use parking_lot::Mutex;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::config::CONFIG;

pub struct VadWrapper {
    vad: Mutex<webrtc_vad::Vad>,
    debounce_count: AtomicUsize,
    threshold: i16,
    in_voice: AtomicBool,
    buffer: Mutex<Vec<f32>>,
}

impl VadWrapper {
    pub fn new() -> Self {
        Self {
            vad: Mutex::new(webrtc_vad::Vad::new_with_rate_and_mode(
                webrtc_vad::SampleRate::Rate16kHz,
                webrtc_vad::VadMode::VeryAggressive,
            )),
            threshold: (CONFIG.vad.threshold_level.abs() * 32767.0).round() as i16,
            debounce_count: AtomicUsize::new(0),
            in_voice: AtomicBool::new(false),
            buffer: Mutex::new(Vec::new()),
        }
    }

    /// Returns debounced voice state for the given audio chunk.
    fn is_voice_segment(&self, data: &[i16]) -> Result<bool, ()> {
        // First check VAD
        let is_voice_vad = self.vad.lock().is_voice_segment(data)?;

        if !is_voice_vad {
            // VAD says not voice, apply debounce
            let mut current = self.debounce_count.load(Ordering::Relaxed);
            loop {
                if current >= CONFIG.vad.debounce_times {
                    return Ok(false);
                }
                match self.debounce_count.compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Successfully incremented from a value < DEBOUNCE_TIMES
                        return Ok(true);
                    }
                    Err(actual) => {
                        current = actual;
                        // Continue loop with updated value
                    }
                }
            }
        }
        
        // VAD says voice, check amplitude threshold
        let threshold = self.threshold;
        let exceeds_threshold = data.iter().any(|&sample| sample.abs() > threshold);
        
        if !exceeds_threshold {
            // Amplitude too low, treat as not voice and apply debounce
            let mut current = self.debounce_count.load(Ordering::Relaxed);
            loop {
                if current >= CONFIG.vad.debounce_times {
                    return Ok(false);
                }
                match self.debounce_count.compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Successfully incremented from a value < DEBOUNCE_TIMES
                        return Ok(true);
                    }
                    Err(actual) => {
                        current = actual;
                        // Continue loop with updated value
                    }
                }
            }
        }
        
        // Both VAD and amplitude threshold indicate voice
        self.debounce_count.store(0, Ordering::Relaxed);
        Ok(true)
    }

    pub fn segment_parse(&self, data: &[i16]) -> Result<VadEvent, ()> {
        let current_voice = self.is_voice_segment(data)?;
        let prev_voice = self.in_voice.load(Ordering::Relaxed);

        match (prev_voice, current_voice) {
            (false, true) => {
                // Voice just started
                self.in_voice.store(true, Ordering::Relaxed);
                let mut buffer = self.buffer.lock();
                buffer.clear();
                // Convert i16 samples to f32 in range [-1.0, 1.0]
                buffer.extend(data.iter().map(|&x| x as f32 / 32768.0));
                Ok(VadEvent::Start)
            }
            (true, true) => {
                // Voice continues, accumulate samples
                let mut buffer = self.buffer.lock();
                buffer.extend(data.iter().map(|&x| x as f32 / 32768.0));
                Ok(VadEvent::Pending)
            }
            (true, false) => {
                // Voice ended after debounce
                self.in_voice.store(false, Ordering::Relaxed);
                let mut buffer = self.buffer.lock();
                let voice_data = std::mem::take(&mut *buffer);
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
    End(Vec<f32>),
}