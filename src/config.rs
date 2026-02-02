use std::{env, fs::File, sync::LazyLock};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub model_path: String,
    pub vad: VadConfig,
    pub language: String,
    pub udp: UdpConfig,
    pub initial_prompt: Option<String>,
}

#[derive(Deserialize)]
pub struct VadConfig {
    pub threshold_level: f32,
    pub debounce_times: usize,
}

#[derive(Deserialize)]
pub struct UdpConfig {
    pub port: u16,
    pub to: String,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| load_config());

fn load_config() -> Config {
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());
    let config_file = File::open(config_path).expect("Failed to open config file");
    let config: Config = serde_json::from_reader(config_file).expect("Failed to parse config file");
    config
}
