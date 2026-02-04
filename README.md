# VRC-STT-RS

<img src="./arts/vrc-stt-rs.svg" width="256">

Real-time speech-to-text for VRChat using Whisper. Captures microphone input, transcribes speech with OpenAI's Whisper model, and sends text to VRChat via OSC.

[中文版本](./README.zh_CN.md) | [日本語バージョン](./README.ja_JP.md)

## Quick Start

1. Download the [latest release](https://github.com/BERADQ/vrc-stt-rs/releases)
2. Get a Whisper model:
   ```bash
   mkdir model
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
   ```
3. Enable OSC in VRChat: `Settings → OSC → Enabled`
4. Run: `./vrc-stt-rs`

## Features

- Real-time voice transcription with Whisper models
- Voice Activity Detection (VAD) for automatic speech recognition
- OSC integration for seamless VRChat compatibility
- Multi-language support
- Configurable via GUI settings

## Requirements

- OS: Windows or Linux
- Audio: Working microphone
- Memory: 1GB minimum (4GB+ recommended, or GPU memory for accelerated models)
- Storage: 1-2GB for models

## Configuration

Select your preferred model, language, and audio settings in the app's Settings tab. Common Whisper models:
- `tiny.bin`: Fastest, lower accuracy
- `base.bin`: Good balance
- `medium.bin`: Recommended for accuracy
- `large.bin`: Best accuracy, slower

## Roadmap

- [ ] Push-to-talk support
- [ ] Optional automatic model download
- [ ] Recording pause button

## Troubleshooting

No audio? Check microphone permissions and system default device  
No output? Verify model file exists and is ggml format  
OSC not working? Ensure VRChat OSC is enabled and firewall allows port 9000  

Check the Logs tab in-app for detailed error messages.

## License

GPL-3.0 licensed.

Art assets are provided under the [CC BY-NC 4.0](CC-BY-NC-4.0) license. Project icon by 清欢NP_C.

---

**Note**: Unofficial VRChat tool. Use at your own risk.