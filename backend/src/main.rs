use ctrlc;
use rust_i18n::{i18n, t};
use std::sync::mpsc::channel;
use vad::VadWrapper;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};

use crate::{osc::connect_with_config, socket::SocketServer, vad::VadEvent};
use common::{config::ConfigManager, SocketMessage};

fn send_socket_message(socket_server: &SocketServer, msg: SocketMessage) {
    if let Err(e) = socket_server.broadcast(msg) {
        log::warn!("{}", t!("socket.broadcast.failed", error = e));
    }
}

i18n!("../locales", fallback = "en");

mod osc;
mod socket;
mod vad;
mod whisper;

extern "C" fn log_callback<T>(
    _level: T,
    text: *const ::std::os::raw::c_char,
    _user_data: *mut ::std::os::raw::c_void,
) {
    log::debug!("{}", unsafe {
        std::ffi::CStr::from_ptr(text).to_string_lossy()
    })
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    unsafe {
        whisper_rs::set_log_callback(Some(log_callback), std::ptr::null_mut());
    }

    // Initialize socket server
    let (socket_server, broadcast_rx) = SocketServer::new()?;
    let _socket_handle = socket_server.start()?;
    let _broadcaster_handle = socket_server.run_broadcaster(broadcast_rx);

    // Load configuration using ConfigManager
    let config_manager = ConfigManager::load()?;

    rust_i18n::set_locale(&config_manager.config().interface_language);

    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect(&t!("failed.find.input_device"));

    let device_description = device.description()?;
    log::info!("{}", t!("device.name", name = device_description.name()));

    let config = device
        .supported_input_configs()?
        .find(|c| c.min_sample_rate() <= 16000 && c.max_sample_rate() >= 16000)
        .ok_or_else(|| anyhow::anyhow!("{}", t!("no.supported.16kHz")))?
        .with_sample_rate(16000);

    let mut config: StreamConfig = config.into();
    config.buffer_size = cpal::BufferSize::Fixed(16 * 10);
    config.channels = 1;

    let err_fn = |err| log::error!("{}", t!("error.prefix", error = err));

    // Pass config_manager to VadWrapper
    let mut vad = VadWrapper::new(&config_manager);

    let (tx, rx) = channel();

    let stream = device.build_input_stream(
        &config,
        move |data: &[i16], _: &_| match vad.segment_parse(data).unwrap() {
            VadEvent::Start => {
                log::info!("{}", t!("recording.start"));
                tx.send(VadEvent::Start).unwrap();
            }
            VadEvent::Pending => {}
            VadEvent::Recording => {
                tx.send(VadEvent::Recording).unwrap();
            }
            VadEvent::End(voice) => {
                tx.send(VadEvent::End(voice)).unwrap();
            }
        },
        err_fn,
        None,
    )?;

    ctrlc::set_handler(|| {
        println!("{}", t!("ctrlc.signal"));
        std::process::exit(0);
    })
    .expect("Failed to set Ctrl+C handler");

    // Pass config_manager to Whisper
    let mut whisper = whisper::Whisper::new(&config_manager)?;
    // Use config from config_manager for OSC
    let mut osc = connect_with_config(
        ("0.0.0.0", config_manager.config().udp.port),
        config_manager.config(),
    )?;
    while let Ok(event) = rx.recv() {
        match event {
            VadEvent::Start => {
                send_socket_message(&socket_server, SocketMessage::STTRecordStart);
                osc.send_set_typing(true)?;
            }
            VadEvent::End(voice) => {
                let voice_len = voice.len();
                if voice_len > 16 * 10 * 30 {
                    log::info!("{}", t!("recording.end"));
                    // Send processing message first
                    send_socket_message(&socket_server, SocketMessage::STTRecordProcessing);
                    match whisper.transcribe(&voice) {
                        Ok(text) => {
                            log::info!("{}", t!("transcription.complete", text = &text));
                            osc.send_message(&text)?;
                            send_socket_message(
                                &socket_server,
                                SocketMessage::STTRecordEndWithText(text),
                            );
                        }
                        Err(e) => {
                            let error_msg = format!("Transcription failed: {}", e);
                            log::error!("{}", error_msg);
                            send_socket_message(
                                &socket_server,
                                SocketMessage::STTRecordEndWithError(error_msg),
                            );
                        }
                    }
                } else {
                    let error_msg = t!("audio.too.short", length = voice_len);
                    log::error!("{}", error_msg);
                    send_socket_message(
                        &socket_server,
                        SocketMessage::STTRecordEndWithError(error_msg.to_string()),
                    );
                }
                osc.send_set_typing(false)?;
            }
            _ => {}
        }
    }

    stream.play()?;

    Ok(())
}
