use ctrlc;
use eframe::egui;
use rust_i18n::{i18n, t};
use std::{sync::mpsc::channel, thread::JoinHandle};
use vad::VadWrapper;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};

use crate::{config::CONFIG, osc::connect, vad::VadEvent};

i18n!("locales", fallback = "en");

mod config;
mod gui;
mod osc;
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

    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect(&t!("failed.find.input_device"));

    let device_description = device.description()?;
    println!("{}", t!("device.name", name = device_description.name()));

    let config = device
        .supported_input_configs()?
        .find(|c| c.min_sample_rate() <= 16000 && c.max_sample_rate() >= 16000)
        .ok_or_else(|| anyhow::anyhow!("{}", t!("no.supported.16kHz")))?
        .with_sample_rate(16000);

    let mut config: StreamConfig = config.into();
    config.buffer_size = cpal::BufferSize::Fixed(16 * 10);
    config.channels = 1;

    let err_fn = |err| eprintln!("{}", t!("error.prefix", error = err));

    let mut vad = VadWrapper::new();

    let (tx, rx) = channel();

    let stream = device.build_input_stream(
        &config,
        move |data: &[i16], _: &_| match vad.segment_parse(data).unwrap() {
            VadEvent::Start => {
                println!("{}", t!("recording.start"));
                tx.send(VadEvent::Recording).unwrap();
            }
            VadEvent::Pending => {}
            VadEvent::End(voice) => {
                tx.send(VadEvent::End(voice)).unwrap();
            }
            _ => {}
        },
        err_fn,
        None,
    )?;

    let (str_tx, str_rx) = channel();

    let _: JoinHandle<anyhow::Result<()>> = std::thread::spawn(move || {
        let mut whisper = whisper::Whisper::new()?;
        let mut osc = connect(("0.0.0.0", CONFIG.udp.port))?;
        while let Ok(event) = rx.recv() {
            match event {
                VadEvent::End(voice) => {
                    let voice_len = voice.len();
                    if voice_len > 16 * 10 * 30 {
                        println!("{}", t!("recording.end"));
                        let text = whisper.transcribe(&voice)?;
                        println!("{}", t!("transcription.complete", text = &text));
                        osc.send_message(&text)?;
                        str_tx.send(text)?;
                    } else {
                        println!("{}", t!("audio.too.short", length = voice_len));
                    }
                    osc.send_set_typing(false)?;
                }
                VadEvent::Start => {
                    osc.send_set_typing(true)?;
                }
                _ => {}
            }
        }
        Ok(())
    });

    stream.play()?;

    ctrlc::set_handler(|| {
        println!("{}", t!("ctrlc.signal"));
        std::process::exit(0);
    })
    .expect("Failed to set Ctrl+C handler");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "VRC STT",
        options,
        Box::new(|_cc| Ok(Box::new(gui::VrcSttApp::new(str_rx)))),
    )
    .unwrap();

    Ok(())
}
