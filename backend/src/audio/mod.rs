//! Audio capture and processing module
//!
//! Handles audio device selection, configuration, resampling to 16kHz mono,
//! and integration with VAD (Voice Activity Detection).

mod capture;
mod config;
mod resampler;

pub use capture::AudioCapture;
pub use config::AudioConfig;
pub use resampler::AudioResampler;

/// Audio processing error types
#[derive(Debug)]
pub enum AudioError {
    DeviceNotFound,
    ConfigNotSupported,
    StreamError(cpal::BuildStreamError),
    PlayError(cpal::PlayStreamError),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::DeviceNotFound => write!(f, "No input device found"),
            AudioError::ConfigNotSupported => write!(f, "No supported audio configuration found"),
            AudioError::StreamError(e) => write!(f, "Failed to build audio stream: {}", e),
            AudioError::PlayError(e) => write!(f, "Failed to play audio stream: {}", e),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AudioError::StreamError(e) => Some(e),
            AudioError::PlayError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<cpal::BuildStreamError> for AudioError {
    fn from(e: cpal::BuildStreamError) -> Self {
        AudioError::StreamError(e)
    }
}

impl From<cpal::PlayStreamError> for AudioError {
    fn from(e: cpal::PlayStreamError) -> Self {
        AudioError::PlayError(e)
    }
}

/// Result type for audio operations
pub type Result<T> = std::result::Result<T, AudioError>;

/// Chunk size for 16kHz 10ms audio
pub const CHUNK_SIZE: usize = 160;

/// Maximum number of chunks to buffer
pub const MAX_BUFFERED_CHUNKS: usize = 2;

/// Default input sample rate
pub const TARGET_SAMPLE_RATE: u32 = 16000;
