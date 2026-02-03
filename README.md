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

## Installation

#### Download Pre-built Binaries
Download pre-built binaries from [GitHub Releases](https://github.com/BERADQ/vrc-stt-rs/releases) and place it in any directory.

### Download Whisper Models
Download Whisper model files from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp) or other sources and place them in the `model/` directory:

```bash
# Example: Download medium model (recommended for balance of speed/accuracy)
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
```

Available models (size/performance trade-off):
- `tiny.bin` - Fastest, lowest accuracy
- `base.bin` - Good balance for most users
- `small.bin` - Better accuracy
- `medium.bin` - Recommended for good accuracy
- `large.bin` - Best accuracy, slowest



## Usage

### 1. Configure VRChat
Make sure VRChat is running with OSC enabled:
1. Go to **Settings** → **OSC** → **Enabled**
2. Note the OSC port (default: `9000`)

### 2. Run the Application
For development, first build the backend then run the frontend:
```bash
# Build the backend
cargo build --bin backend

# Run the frontend with backend reference
VRC_STT_BACKEND=./target/debug/backend cargo run --bin frontend
```

#### 2.1 Run Pre-built Binary
```bash
./vrc-stt-rs
```

### 3. Using the Software
Once running, the GUI will display:
1. **Status Panel**: Shows current state (Idle, Recording, Processing, Error) with color indicators
2. **History Tab**: Displays transcribed text with timestamps
3. **Settings Tab**: Allows configuration of model, language, VAD, and network settings
4. **Logs Tab**: Shows backend output and error messages
5. **Navigation**: Switch between panels using the top navigation buttons

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
Check logs in the Logs tab of the GUI for backend output and error messages.

## Development

### Adding Features
1. Add new config options to `config.rs` and `config.json`
2. Implement feature in appropriate module
3. Update `Cargo.toml` with new dependencies if needed
4. Test thoroughly before deployment

## License

This project is licensed under the GNU General Public License v3.0 (GPL-3.0) - see the LICENSE file for details.

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

- [ ] Optional microphone configuration
- [ ] Custom hotkeys for manual control

---

**Note**: This software is not affiliated with or endorsed by VRChat Inc. Use at your own risk.