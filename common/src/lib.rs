use serde::{Deserialize, Serialize};

/// Message types for communication between backend and frontend via Unix socket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SocketMessage {
    /// STT recording started
    STTRecordStart,
    /// STT recording ended, processing transcribed text
    STTRecordProcessing,
    /// STT recording ended successfully with transcribed text
    STTRecordEndWithText(String),
    /// STT recording ended with an error
    STTRecordEndWithError(String),
    /// Ping to keep connection alive
    Ping,
    /// Pong response
    Pong,
}

impl SocketMessage {
    /// Serialize message to JSON string with newline delimiter
    pub fn to_json(&self) -> anyhow::Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(format!("{}\n", json))
    }

    /// Deserialize message from JSON string
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let msg = serde_json::from_str(json)?;
        Ok(msg)
    }
}

/// Default Unix socket path
pub fn default_socket_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("vrc-stt.sock");
    path
}

/// Configuration types
pub mod config;
pub use config::{AudioConfig, ChannelMixMode, Config, ConfigManager, UdpConfig, VadConfig};

pub mod lang;
pub use lang::ALL_LANG;
