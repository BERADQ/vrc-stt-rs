//! Audio capture stream
//!
//! Builds and manages the CPAL input stream for audio capture.

use arrayvec::ArrayVec;
use cpal::{
    BufferSize, Device, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use rust_i18n::t;
use std::cell::RefCell;
use std::sync::mpsc::Sender;

use crate::vad::{VadEvent, VadWrapper};

use super::{
    AudioConfig, AudioResampler, CHUNK_SIZE, MAX_BUFFERED_CHUNKS, Result, TARGET_SAMPLE_RATE,
};
use common::ChannelMixMode;

thread_local! {
    static RESAMPLE_BUFFER: RefCell<ArrayVec<f32, { MAX_BUFFERED_CHUNKS * CHUNK_SIZE }>> =
        RefCell::new(ArrayVec::new());
}

/// Audio capture stream handler
pub struct AudioCapture {
    device: Device,
    config: AudioConfig,
    mix_mode: ChannelMixMode,
}

impl AudioCapture {
    /// Create a new audio capture instance with the specified channel mix mode
    pub fn new(mix_mode: ChannelMixMode) -> Result<Self> {
        let config = AudioConfig::from_default_device()?;

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(super::AudioError::DeviceNotFound)?;

        Ok(Self {
            device,
            config,
            mix_mode,
        })
    }

    /// Build the input stream with VAD processing
    ///
    /// This method is designed to be called once. The `vad` is moved into the stream callback.
    pub fn build_stream(&self, vad: VadWrapper, event_sender: Sender<VadEvent>) -> Result<Stream> {
        let mut stream_config: StreamConfig = self.config.supported_config().clone().into();
        stream_config.channels = self.config.channels();

        // Set buffer size for 10ms chunks
        let buffer_size_frames = (self.config.sample_rate() * 10) / 1000;
        stream_config.buffer_size = BufferSize::Fixed(buffer_size_frames);

        log::info!(
            "{}",
            t!(
                "buffer.size.set",
                frames = buffer_size_frames,
                rate = self.config.sample_rate()
            )
        );

        log::info!(
            "{}",
            t!(
                "using.sample_rate",
                rate = self.config.sample_rate(),
                target = TARGET_SAMPLE_RATE
            )
        );

        // Create resampler if needed
        let needs_processing = self.config.needs_processing();

        if needs_processing {
            log::info!(
                "{}",
                t!("resampler.initializing", from = self.config.sample_rate())
            );
        }

        let sample_rate = self.config.sample_rate();
        let channels = self.config.channels() as usize;

        // Wrap resampler and vad in RefCell for interior mutability
        let mix_mode = self.mix_mode;
        let resampler: RefCell<Option<AudioResampler>> = RefCell::new(if needs_processing {
            AudioResampler::new(sample_rate, channels, mix_mode)
                .ok()
                .flatten()
        } else {
            None
        });
        let vad = RefCell::new(vad);

        // Build the stream
        let stream = self.device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| {
                if let Some(ref mut r) = *resampler.borrow_mut() {
                    process_with_resampling(data, r, &mut *vad.borrow_mut(), &event_sender);
                } else {
                    for chunk in data.chunks_exact(CHUNK_SIZE) {
                        send_chunk_to_vad(chunk, &mut *vad.borrow_mut(), &event_sender);
                    }
                }
            },
            |err| log::error!("{}", t!("error.prefix", error = err)),
            None,
        )?;

        Ok(stream)
    }
}

/// Process audio with resampling
fn process_with_resampling(
    data: &[f32],
    resampler: &mut AudioResampler,
    vad: &mut VadWrapper,
    sender: &Sender<VadEvent>,
) {
    RESAMPLE_BUFFER.with(|buf| {
        let mut buffer = buf.borrow_mut();

        match resampler.resample(data) {
            Ok(resampled) => {
                if resampled.is_empty() {
                    // Not enough data accumulated yet, wait for next callback
                    return;
                }

                // Append to buffer
                for sample in resampled {
                    if buffer.try_push(sample).is_err() {
                        // Buffer full, process oldest chunk
                        let old_chunk: [f32; CHUNK_SIZE] = std::array::from_fn(|i| buffer[i]);
                        send_chunk_to_vad(&old_chunk, vad, sender);
                        buffer.drain(..CHUNK_SIZE);
                        buffer.push(sample);
                    }
                }

                // Process complete chunks
                while buffer.len() >= CHUNK_SIZE {
                    let chunk: [f32; CHUNK_SIZE] = std::array::from_fn(|i| buffer[i]);
                    send_chunk_to_vad(&chunk, vad, sender);
                    buffer.drain(..CHUNK_SIZE);
                }
            }
            Err(e) => {
                log::error!("{}", t!("resample.error", error = e));
            }
        }
    });
}

/// Send a single chunk to VAD and forward events
fn send_chunk_to_vad(chunk: &[f32], vad: &mut VadWrapper, sender: &Sender<VadEvent>) {
    if chunk.len() != CHUNK_SIZE {
        return;
    }

    match vad.segment_parse(chunk) {
        VadEvent::Start => {
            log::info!("{}", t!("recording.start"));
            let _ = sender.send(VadEvent::Start);
        }
        VadEvent::Recording => {
            let _ = sender.send(VadEvent::Recording);
        }
        VadEvent::End(voice) => {
            let _ = sender.send(VadEvent::End(voice));
        }
        VadEvent::Pending => {}
    }
}
