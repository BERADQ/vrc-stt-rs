//! VRC STT Backend
//!
//! Real-time speech-to-text for VRChat using Whisper.
//! Captures microphone input, transcribes speech, and sends text via OSC.

use cpal::traits::StreamTrait;
use ctrlc;
use rust_i18n::{i18n, t};
use std::sync::mpsc::channel;

use common::{SocketMessage, config::ConfigManager};

use crate::{
    audio::AudioCapture,
    osc::VrcOsc,
    socket::{SocketClient, create_default_client},
    vad::{VadEvent, VadWrapper},
    whisper::Whisper,
};

mod audio;
mod osc;
mod socket;
mod vad;
mod whisper;

i18n!("../locales", fallback = "en");

/// Minimum voice length: 0.3 seconds at 16kHz
const MIN_VOICE_SAMPLES: usize = 16000 * 3 / 10;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();
    setup_whisper_logging();

    // Initialize socket client
    let socket = create_default_client()?;

    // Load configuration
    let config_manager = ConfigManager::load()?;
    rust_i18n::set_locale(&config_manager.config().interface_language);

    // Initialize components
    let mix_mode = config_manager.config().audio.channel_mix_mode.clone();
    log::info!("Audio channel mix mode: {}", mix_mode.as_str());
    let audio = AudioCapture::new(mix_mode)?;
    let vad = VadWrapper::new(&config_manager);
    let mut whisper = Whisper::new(&config_manager)?;
    let mut osc = VrcOsc::from_config(config_manager.config())?;

    // Setup Ctrl+C handler
    ctrlc::set_handler(|| {
        println!("{}", t!("ctrlc.signal"));
        std::process::exit(0);
    })?;

    // Create audio processing channel
    let (tx, rx) = channel();

    // Build and start audio stream
    let stream = audio.build_stream(vad, tx)?;
    stream.play()?;

    log::info!("VRC STT started, waiting for voice input...");

    // Main event loop
    for event in rx {
        match event {
            VadEvent::Start => {
                send_socket(&socket, SocketMessage::STTRecordStart);
                osc.set_typing(true)?;
            }
            VadEvent::End(voice) => {
                handle_voice_end(&voice, &mut whisper, &mut osc, &socket)?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Handle voice recording end
fn handle_voice_end(
    voice: &[f32],
    whisper: &mut Whisper,
    osc: &mut VrcOsc,
    socket: &SocketClient,
) -> anyhow::Result<()> {
    // Check minimum voice length
    if voice.len() < MIN_VOICE_SAMPLES {
        let error_msg = t!("audio.too.short", length = voice.len());
        log::warn!("{}", error_msg);
        send_socket(
            socket,
            SocketMessage::STTRecordEndWithError(error_msg.to_string()),
        );
        osc.set_typing(false)?;
        return Ok(());
    }

    log::info!("{}", t!("recording.end"));
    send_socket(socket, SocketMessage::STTRecordProcessing);

    // Transcribe
    match whisper.transcribe(voice) {
        Ok(text) => {
            log::info!("{}", t!("transcription.complete", text = &text));
            osc.send_message(&text)?;
            send_socket(socket, SocketMessage::STTRecordEndWithText(text));
        }
        Err(e) => {
            let error_msg = format!("Transcription failed: {}", e);
            log::error!("{}", error_msg);
            send_socket(socket, SocketMessage::STTRecordEndWithError(error_msg));
        }
    }

    osc.set_typing(false)?;
    Ok(())
}

/// Send message to socket (logs warnings on failure)
fn send_socket(socket: &SocketClient, msg: SocketMessage) {
    if let Err(e) = socket.send(&msg) {
        log::warn!("{}", t!("socket.broadcast.failed", error = e));
    }
}

/// Whisper logging callback
/// On linux _level is a u32, on windows it's a i32
extern "C" fn whisper_log_callback<T>(
    _level: T,
    text: *const std::os::raw::c_char,
    _user_data: *mut std::os::raw::c_void,
) {
    let msg = unsafe { std::ffi::CStr::from_ptr(text).to_string_lossy() };
    log::debug!("{}", msg);
}

/// Setup whisper-rs logging callback
fn setup_whisper_logging() {
    unsafe {
        whisper_rs::set_log_callback(Some(whisper_log_callback), std::ptr::null_mut());
    }
}
