use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// Application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub model_path: String,
    pub vad: VadConfig,
    pub language: String,
    pub udp: UdpConfig,
    pub initial_prompt: Option<String>,
    pub interface_language: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_path: "./model/medium.bin".to_owned(),
            vad: VadConfig::default(),
            language: "en".to_owned(),
            udp: UdpConfig::default(),
            initial_prompt: None,
            interface_language: "en".to_owned(),
        }
    }
}

/// VAD (Voice Activity Detection) configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VadConfig {
    pub threshold_level: f32,
    pub debounce_times: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold_level: 0.1,
            debounce_times: 160,
        }
    }
}

/// UDP configuration for OSC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UdpConfig {
    pub port: u16,
    pub to: String,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            port: 5005,
            to: "127.0.0.1:9000".to_owned(),
        }
    }
}

/// Manager for configuration loading and saving
#[derive(Clone)]
pub struct ConfigManager {
    config: Config,
    path: PathBuf,
}

impl ConfigManager {
    /// Create a new ConfigManager with the given configuration and path
    pub fn new(config: Config, path: PathBuf) -> Self {
        Self { config, path }
    }

    /// Load configuration from the system config path
    pub fn load() -> anyhow::Result<Self> {
        let path = system_config_path();
        Self::load_from(&path)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content)?
        } else {
            let config = Config::default();
            let manager = Self::new(config.clone(), path.clone());
            manager.save()?;
            config
        };
        Ok(Self::new(config, path))
    }

    /// Save the current configuration to file
    pub fn save(&self) -> anyhow::Result<()> {
        let dir_path = self.path.parent().ok_or(anyhow::anyhow!("Invalid path"))?;
        std::fs::create_dir_all(dir_path)?;
        let content = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Get the path where the configuration is stored
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let new_manager = Self::load_from(&self.path)?;
        self.config = new_manager.config;
        Ok(())
    }

    /// Update configuration and save to disk
    pub fn update(&mut self, config: Config) -> anyhow::Result<()> {
        self.config = config;
        self.save()
    }
}

impl Config {
    /// Load configuration from file (legacy API)
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file (legacy API)
    pub fn to_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let manager = ConfigManager::new(self.clone(), path.as_ref().to_path_buf());
        manager.save()
    }
}

/// Global configuration instance
static CONFIG: LazyLock<ConfigManager> =
    LazyLock::new(|| ConfigManager::load().expect("Failed to load configuration"));

/// Get a reference to the global configuration
pub fn config() -> &'static Config {
    CONFIG.config()
}

/// Get a mutable reference to the global configuration manager
/// Note: This requires synchronization if used in multi-threaded contexts
pub fn config_manager() -> &'static ConfigManager {
    &CONFIG
}

pub fn system_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap();
    path.push("vrc-stt");
    path.push("config.json");
    path
}
