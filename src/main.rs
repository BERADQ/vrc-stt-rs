use ctrlc;
use std::sync::mpsc::channel;
use vad::VadWrapper;

use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{config::CONFIG, osc::connect, vad::VadEvent};

mod config;
mod osc;
mod vad;
mod whisper;

extern "C" fn log_callback(
    _level: u32,
    _text: *const ::std::os::raw::c_char,
    _user_data: *mut ::std::os::raw::c_void,
) {
}

fn main() -> anyhow::Result<()> {
    unsafe {
        whisper_rs::set_log_callback(Some(log_callback), std::ptr::null_mut());
    }

    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect("Failed to find input device");

    let config = device
        .supported_input_configs()?
        .find(|c| c.min_sample_rate() <= 16000 && c.max_sample_rate() >= 16000)
        .ok_or_else(|| anyhow::anyhow!("No input config supporting 16000 Hz"))?
        .with_sample_rate(16000);
    let mut config: StreamConfig = config.into();
    config.buffer_size = cpal::BufferSize::Fixed(16 * 10);
    config.channels = 1;

    let err_fn = |err| eprintln!("Error: {}", err);

    let vad = VadWrapper::new();

    let (tx, rx) = channel();

    let stream = device.build_input_stream(
        &config,
        move |data: &[i16], _: &_| match vad.segment_parse(data).unwrap() {
            VadEvent::Start => println!("Start recording"),
            VadEvent::Pending => {}
            VadEvent::End(voice) => {
                let voice_len = voice.len();
                if voice_len > 16 * 10 * 30 {
                    println!("Recording ended.",);
                    tx.send(voice).unwrap();
                } else {
                    println!(
                        "Recording ended, audio too short ({} samples), ignoring",
                        voice_len
                    );
                }
            }
        },
        err_fn,
        None,
    )?;

    std::thread::spawn(move || {
        let mut whisper = whisper::Whisper::new().unwrap();
        let mut osc = connect(("0.0.0.0", CONFIG.udp.port)).unwrap();
        while let Ok(voice) = rx.recv() {
            println!(
                "Starting transcription, audio length {} samples",
                voice.len()
            );
            let text = whisper.transcribe(&voice).unwrap();
            println!("Transcription complete: {}", text);
            osc.send_message(&text).unwrap();
        }
    });

    stream.play()?;

    ctrlc::set_handler(|| {
        println!("Received Ctrl+C signal, exiting...");
        std::process::exit(0);
    })
    .expect("Failed to set Ctrl+C handler");

    // Block main thread until signal is received
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
