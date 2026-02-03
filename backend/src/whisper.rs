use rust_i18n::t;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use common::config::ConfigManager;

pub struct Whisper {
    ctx: WhisperContext,
    config_manager: ConfigManager,
}

impl Whisper {
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
        Ok(Whisper {
            ctx,
            config_manager: config_manager.clone(),
        })
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> anyhow::Result<String> {
        let config = self.config_manager.config();
        let mut state = self.ctx.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        if let Some(initial_prompt) = &config.initial_prompt {
            params.set_initial_prompt(initial_prompt);
        }
        params.set_language(Some(&config.language));
        params.set_translate(false);
        params.set_no_timestamps(true);
        params.set_no_context(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(true);
        // params.set_vad_model_path(Some(&config.vad.model_path));
        // params.enable_vad(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        state.full(params, audio)?;

        let mut result = String::new();
        for segment in state.as_iter() {
            match segment.to_str() {
                Ok(text) => {
                    result.push_str(text);
                }
                Err(e) => {
                    eprintln!("{}", t!("transcription.segment.error", error = e));
                }
            }
        }

        if result.is_empty() {
            eprintln!("{}", t!("no.transcription.result"));
        }

        Ok(result)
    }
}
