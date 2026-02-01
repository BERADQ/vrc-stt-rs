use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::config::CONFIG;

pub struct Whisper {
    ctx: WhisperContext,
}

impl Whisper {
    pub fn new() -> anyhow::Result<Self> {
        let ctx = WhisperContext::new_with_params(
            &CONFIG.model_path,
            WhisperContextParameters::default(),
        )?;
        Ok(Whisper { ctx })
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> anyhow::Result<String> {
        let mut state = self.ctx.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        params.set_initial_prompt(&CONFIG.initial_prompt);
        params.set_language(Some(&CONFIG.language));
        params.set_translate(false);
        params.set_no_timestamps(true);
        params.set_no_context(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(true);
        // params.set_vad_model_path(Some(&CONFIG.vad.model_path));
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
                    eprintln!("Error getting text for segment {:?}", e);
                }
            }
        }

        if result.is_empty() {
            eprintln!("No transcription result");
        }

        Ok(result)
    }
}
