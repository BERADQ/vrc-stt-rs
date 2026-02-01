# VRC-STT-RS

A minimal, high-performance Speech-to-Text (STT) software for VRChat, written in Rust. This tool captures your microphone input, transcribes speech in real-time using OpenAI's Whisper model, and sends the transcribed text to VRChat via OSC (Open Sound Control).

[中文版本](./README.zh_CN.md)


## Features

- **Voice Activity Detection (VAD)** using WebRTC's VAD algorithm with configurable thresholds
- **Speech transcription** using OpenAI's Whisper models
- **OSC integration** for seamless communication with VRChat
- **Configurable parameters** through JSON configuration
- **Multi-language support** (configured via settings)

## Prerequisites

### System Requirements
- **Operating System**: Windows, Linux
- **Audio**: Working microphone input
- **RAM**: Minimum 4GB, 8GB+ recommended for larger Whisper models
- **Storage**: ~1-2GB for Whisper model files

## Build & Installation

### 1. Clone the Repository
```bash
git clone https://github.com/BERADQ/vrc-stt-rs.git
cd vrc-stt-rs
```

### 2. Build
```bash
cargo build --release
```

#### 3.1 Or Download Pre-built Binaries
Download pre-built binaries from [GitHub Releases](https://github.com/BERADQ/vrc-stt-rs/releases) and place it in any directory.

### 3. Download Whisper Models
Download Whisper model files from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp) or other sources and place them in the `model/` directory:

```bash
# Example: Download medium model (recommended for balance of speed/accuracy)
wget -P model/ https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
```

Available models (size/performance trade-off):
- `tiny.bin` - Fastest, lowest accuracy
- `base.bin` - Good balance for most users
- `small.bin` - Better accuracy
- `medium.bin` - Recommended for good accuracy
- `large.bin` - Best accuracy, slowest

## Configuration

Edit `config.json` to customize the behavior:

```json
{
  // Path to Whisper model file
  "model_path": "./model/medium.bin",
  // Voice Activity Detection (VAD) settings
  "vad": {
    "threshold_level": 0.05,
    "debounce_times": 32
  },
  // Change language code for transcription
  "language": "zh",
  // Initial prompt for Whisper
  "initial_prompt": "使用简体中文输出",
  "udp": {
    "port": 5005,
    // TO VRChat OSC address
    "to": "127.0.0.1:9000"
  }
}
```

### Environment Variables
- `CONFIG_PATH`: Override the default config file location

## Usage

### 1. Configure VRChat
Make sure VRChat is running with OSC enabled:
1. Go to **Settings** → **OSC** → **Enabled**
2. Note the OSC port (default: `9000`)

### 2. Run the Application
```bash
# Development mode (with debugging)
cargo run

# Release mode (optimized)
cargo run --release

# With custom config path
CONFIG_PATH=./myconfig.json cargo run --release
```

#### 2.1 Run Pre-built Binary
```bash
# With default config path
./vrc-stt-rs

# With custom config path
CONFIG_PATH=./myconfig.json ./vrc-stt-rs
```

### 3. Using the Software
Once running, the software will:
1. Monitor your microphone for speech
2. Display messages when recording starts/stops
3. Transcribe speech and send to VRChat
4. Print transcriptions to console

Example output:
```
Start recording
Recording ended, audio length 5120 samples, transcribing...
Starting transcription, audio length 5120 samples
Transcription complete: Hello VRChat!
```

### 4. Stopping the Application
Press `Ctrl+C` to stop the application.

## How It Works

## Troubleshooting

### Common Issues

#### No Audio Input
- Check your default microphone in system settings
- Verify the application has microphone permissions
- Test with `arecord` (Linux) or system recording tools

#### No Transcription Output
- Verify Whisper model file exists at configured path
- Check model file is compatible (ggml format)
- Ensure sufficient RAM for model size

#### OSC Not Reaching VRChat
- Confirm VRChat OSC is enabled in settings
- Check firewall allows UDP traffic on port 9000
- Verify IP address in config matches your system

#### High CPU Usage
- Use smaller Whisper model (tiny/base instead of medium/large)
- Consider GPU acceleration with CUDA/ROCm or any other hardware acceleration
- Adjust VAD debounce_times higher

### Debugging
Enable verbose logging by running in development mode:
```bash
RUST_LOG=debug cargo run
```

Check audio device configuration:
```bash
# List audio devices
cargo run --features=cpal/debug
```

## Performance Tuning

### For Better Accuracy
- Use larger Whisper model (medium/large)
- Lower VAD threshold_level (e.g., 0.02)
- Increase debounce_times (e.g., 64)

### For Lower Latency
- Use smaller Whisper model (tiny/base)
- Higher VAD threshold_level (e.g., 0.1)
- Decrease debounce_times (e.g., 16)

## Development

### Building from Source
```bash
# Debug build
cargo build
```

### Adding Features
1. Add new config options to `config.rs` and `config.json`
2. Implement feature in appropriate module
3. Update `Cargo.toml` with new dependencies if needed
4. Test thoroughly before deployment

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [OpenAI Whisper](https://github.com/openai/whisper) for the speech recognition model
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) for the C++ implementation
- [whisper-rs](https://github.com/tazz4843/whisper-rs) for Rust bindings
- [WebRTC VAD](https://github.com/daily-co/webrtc-vad) for voice activity detection
- [rosc](https://github.com/keschwa/rosc) for OSC implementation
- [cpal](https://github.com/RustAudio/cpal) for cross-platform audio I/O

## Support

If you encounter issues:
1. Check the [Troubleshooting](#troubleshooting) section
2. Search existing issues
3. Create a new issue with:
   - Your configuration
   - Error messages
   - Steps to reproduce
   - System information

## Roadmap

- [ ] Multiple microphone support
- [ ] Custom hotkeys for manual control

---

**Note**: This software is not affiliated with or endorsed by VRChat Inc. Use at your own risk.