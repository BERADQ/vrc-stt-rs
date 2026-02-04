//! Whisper speech-to-text transcription
//!
//! Provides transcription using OpenAI's Whisper model via whisper-rs.

use rust_i18n::t;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use common::config::ConfigManager;

/// Whisper transcription engine
pub struct Whisper {
    ctx: WhisperContext,
    config_manager: ConfigManager,
}

impl Whisper {
    /// Create a new Whisper instance
    pub fn new(config_manager: &ConfigManager) -> anyhow::Result<Self> {
        log::info!(
            "{}",
            t!(
                "whisper.init",
                model_path = config_manager.config().model_path
            )
        );

        let ctx = WhisperContext::new_with_params(
            &config_manager.config().model_path,
            WhisperContextParameters::default(),
        )?;

        Ok(Self {
            ctx,
            config_manager: config_manager.clone(),
        })
    }

    /// Transcribe audio to text
    ///
    /// Audio must be 16kHz mono f32 samples in range [-1.0, 1.0]
    pub fn transcribe(&mut self, audio: &[f32]) -> anyhow::Result<String> {
        let config = self.config_manager.config();
        let mut state = self.ctx.create_state()?;

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        // Set language
        params.set_language(Some(&config.language));

        // Set initial prompt if configured
        if let Some(prompt) = &config.initial_prompt {
            params.set_initial_prompt(prompt);
        }

        // Configure output
        params.set_translate(false);
        params.set_no_timestamps(true);
        params.set_no_context(true);
        params.set_single_segment(true);

        // Disable debug output
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Quality settings
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        // Run transcription
        state.full(params, audio)?;

        // Collect results
        let mut result = String::new();
        for segment in state.as_iter() {
            match segment.to_str() {
                Ok(text) => result.push_str(text),
                Err(e) => {
                    log::warn!("{}", t!("transcription.segment.error", error = e));
                }
            }
        }

        if result.is_empty() {
            log::warn!("{}", t!("no.transcription.result"));
        }

        Ok(result)
    }
}
