use ctrlc;
use rust_i18n::{i18n, t};
use std::sync::mpsc::channel;
use vad::VadWrapper;

use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{osc::connect_with_config, socket::SocketServer, vad::VadEvent};
use common::{SocketMessage, config::ConfigManager};

fn send_socket_message(socket_server: &SocketServer, msg: SocketMessage) {
    if let Err(e) = socket_server.broadcast(msg) {
        log::warn!("{}", t!("socket.broadcast.failed", error = e));
    }
}

i18n!("../locales", fallback = "en");

mod osc;
mod resampler;
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

/// Find the best supported sample rate: prefer 16kHz, fallback to 24kHz
fn find_best_config(device: &cpal::Device) -> anyhow::Result<(cpal::SupportedStreamConfig, u32)> {
    let supported_configs: Vec<_> = device.supported_input_configs()?.collect();

    // First try to find 16kHz support
    if let Some(config) = supported_configs
        .iter()
        .find(|c| c.min_sample_rate() <= 16000 && c.max_sample_rate() >= 16000)
    {
        log::info!("{}", t!("found.16kHz.support"));
        return Ok((config.with_sample_rate(16000), 16000));
    }

    // Fallback to 24kHz
    if let Some(config) = supported_configs
        .iter()
        .find(|c| c.min_sample_rate() <= 24000 && c.max_sample_rate() >= 24000)
    {
        log::info!("{}", t!("fallback.24kHz.using"));
        return Ok((config.with_sample_rate(24000), 24000));
    }

    // Try other common sample rates that can be resampled
    for &rate in &[48000u32, 44100, 32000, 22050] {
        if let Some(config) = supported_configs
            .iter()
            .find(|c| c.min_sample_rate() <= rate && c.max_sample_rate() >= rate)
        {
            log::info!("{}", t!("fallback.other_rate.using", rate = rate));
            return Ok((config.with_sample_rate(rate), rate));
        }
    }

    Err(anyhow::anyhow!("{}", t!("no.supported.sample_rate")))
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

    // Find best supported config and actual sample rate
    let (supported_config, actual_sample_rate) = find_best_config(&device)?;
    log::info!(
        "{}",
        t!(
            "using.sample_rate",
            rate = actual_sample_rate,
            target = 16000
        )
    );

    let mut config: StreamConfig = supported_config.into();
    config.channels = 1;

    // Set buffer size based on actual sample rate (10ms chunks)
    let buffer_size_frames = (actual_sample_rate * 10) / 1000;
    config.buffer_size = cpal::BufferSize::Fixed(buffer_size_frames);
    log::info!(
        "{}",
        t!(
            "buffer.size.set",
            frames = buffer_size_frames,
            rate = actual_sample_rate
        )
    );

    let err_fn = |err| log::error!("{}", t!("error.prefix", error = err));

    // Create resampler if needed
    let needs_resampling = actual_sample_rate != 16000;
    let mut resampler = if needs_resampling {
        log::info!(
            "{}",
            t!("resampler.initializing", from = actual_sample_rate)
        );
        Some(resampler::AudioResampler::new(actual_sample_rate)?)
    } else {
        None
    };

    // Pass config_manager to VadWrapper
    let mut vad = VadWrapper::new(&config_manager);

    let (tx, rx) = channel();

    // Fixed-size ring buffer for resampled data (max 2 chunks = 320 samples)
    // Using ArrayVec to avoid heap allocation
    const MAX_BUFFERED_CHUNKS: usize = 2;
    const CHUNK_SIZE: usize = 160;
    let mut resample_buffer: arrayvec::ArrayVec<f32, { MAX_BUFFERED_CHUNKS * CHUNK_SIZE }> =
        arrayvec::ArrayVec::new();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &_| {
            // Helper function to process a single chunk
            let process_chunk = |chunk: &[f32], vad: &mut VadWrapper, tx: &std::sync::mpsc::Sender<VadEvent>| {
                if chunk.len() != CHUNK_SIZE {
                    return;
                }
                match vad.segment_parse(chunk).unwrap() {
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
                }
            };

            if let Some(ref mut r) = resampler {
                // Resampling path
                match r.resample(data) {
                    Ok(resampled) => {
                        if resampled.is_empty() {
                            return;
                        }
                        // Append to ring buffer
                        for sample in resampled {
                            if resample_buffer.try_push(sample).is_err() {
                                // Buffer full, process oldest chunk first
                                let old_chunk: [f32; CHUNK_SIZE] = std::array::from_fn(|i| resample_buffer[i]);
                                process_chunk(&old_chunk, &mut vad, &tx);
                                // Remove oldest chunk by shifting
                                resample_buffer.drain(..CHUNK_SIZE);
                                resample_buffer.push(sample);
                            }
                        }
                        // Process complete chunks
                        while resample_buffer.len() >= CHUNK_SIZE {
                            let chunk: [f32; CHUNK_SIZE] = std::array::from_fn(|i| resample_buffer[i]);
                            process_chunk(&chunk, &mut vad, &tx);
                            resample_buffer.drain(..CHUNK_SIZE);
                        }
                    }
                    Err(e) => {
                        log::error!("{}", t!("resample.error", error = e));
                        return;
                    }
                }
            } else {
                // No resampling - process directly from input
                // For 16kHz, data should already be in 160-sample chunks
                for chunk in data.chunks_exact(CHUNK_SIZE) {
                    process_chunk(chunk, &mut vad, &tx);
                }
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
    stream.play()?;

    while let Ok(event) = rx.recv() {
        match event {
            VadEvent::Start => {
                send_socket_message(&socket_server, SocketMessage::STTRecordStart);
                osc.send_set_typing(true)?;
            }
            VadEvent::End(voice) => {
                let voice_len = voice.len();
                // Minimum 0.3 seconds at 16kHz (4800 samples)
                if voice_len > 16000 * 3 / 10 {
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

    Ok(())
}
