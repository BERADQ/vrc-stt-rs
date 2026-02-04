//! Audio device configuration
//!
//! Finds the best supported audio configuration from the input device.

use cpal::{
    Device, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use rust_i18n::t;

use super::{AudioError, Result, TARGET_SAMPLE_RATE};

/// Audio configuration for the input device
#[derive(Debug, Clone)]
pub struct AudioConfig {
    supported_config: SupportedStreamConfig,
    actual_sample_rate: u32,
    channels_to_use: u16,
    needs_channel_mix: bool,
    needs_resampling: bool,
}

impl AudioConfig {
    /// Create a new audio configuration from the default input device
    pub fn from_default_device() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::DeviceNotFound)?;

        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        log::info!("{}", t!("device.name", name = device_name));

        Self::from_device(&device)
    }

    /// Create audio configuration from a specific device
    pub fn from_device(device: &Device) -> Result<Self> {
        let supported_configs: Vec<_> = device
            .supported_input_configs()
            .map_err(|_| AudioError::ConfigNotSupported)?
            .collect();

        // Try to find best configuration (prefer 16kHz mono)
        let (config, sample_rate, channels) =
            find_best_stream_config(&supported_configs)?;

        let needs_channel_mix = channels > 1;
        let needs_resampling = sample_rate != TARGET_SAMPLE_RATE;

        if needs_channel_mix {
            log::info!(
                "Device using {} channels, will mix to mono",
                channels
            );
        }

        Ok(Self {
            supported_config: config,
            actual_sample_rate: sample_rate,
            channels_to_use: channels,
            needs_channel_mix,
            needs_resampling,
        })
    }

    /// Get the supported stream configuration
    pub fn supported_config(&self) -> &SupportedStreamConfig {
        &self.supported_config
    }

    /// Get the actual sample rate
    pub fn sample_rate(&self) -> u32 {
        self.actual_sample_rate
    }

    /// Get the number of channels to use
    pub fn channels(&self) -> u16 {
        self.channels_to_use
    }

    /// Check if any processing (resampling or mixing) is needed
    pub fn needs_processing(&self) -> bool {
        self.needs_resampling || self.needs_channel_mix
    }
}

/// Find the best supported sample rate and channel configuration
///
/// Priority:
/// 1. 16kHz mono (ideal for Whisper)
/// 2. 16kHz any channels
/// 3. Other sample rates that can be resampled
///
/// Returns: (supported_config, actual_sample_rate, channels_to_use)
fn find_best_stream_config(
    configs: &[cpal::SupportedStreamConfigRange],
) -> Result<(SupportedStreamConfig, u32, u16)> {
    // Try 16kHz first, preferring mono
    if let Some(config) = find_config_at_rate(configs, TARGET_SAMPLE_RATE) {
        log::info!("{}", t!("found.16kHz.support"));
        let channels = config.channels();
        let channels_to_use = if channels == 1 { 1 } else { channels };
        return Ok((config.with_sample_rate(TARGET_SAMPLE_RATE), TARGET_SAMPLE_RATE, channels_to_use));
    }

    // Try 24kHz
    if let Some(config) = find_config_at_rate(configs, 24000) {
        log::info!("{}", t!("fallback.24kHz.using"));
        let channels = config.channels();
        let channels_to_use = if channels == 1 { 1 } else { channels };
        return Ok((config.with_sample_rate(24000), 24000, channels_to_use));
    }

    // Try other common rates
    for &rate in &[48000u32, 44100, 32000, 22050] {
        if let Some(config) = find_config_at_rate(configs, rate) {
            log::info!("{}", t!("fallback.other_rate.using", rate = rate));
            let channels = config.channels();
            let channels_to_use = if channels == 1 { 1 } else { channels };
            return Ok((config.with_sample_rate(rate), rate, channels_to_use));
        }
    }

    Err(AudioError::ConfigNotSupported)
}

/// Find a configuration that supports the given sample rate
fn find_config_at_rate(
    configs: &[cpal::SupportedStreamConfigRange],
    rate: u32,
) -> Option<&cpal::SupportedStreamConfigRange> {
    configs
        .iter()
        .filter(|c| c.min_sample_rate() <= rate && c.max_sample_rate() >= rate)
        .min_by_key(|c| c.channels())
}
