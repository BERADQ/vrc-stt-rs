# VRC-STT-RS

<img src="./arts/vrc-stt-rs.svg" width="256">

适用于 VRChat 的实时语音转文字工具，基于 Whisper 实现。捕捉麦克风输入，使用 OpenAI 的 Whisper 模型实时转录语音，并通过 OSC 发送文字到 VRChat。

[English Version](./README.md)

## 快速开始

1. 下载 [最新版本](https://github.com/BERADQ/vrc-stt-rs/releases)
2. 获取 Whisper 模型:
   ```bash
   mkdir model
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
   ```
3. 在 VRChat 中启用 OSC: `设置 → OSC → 启用`
4. 运行: `./vrc-stt-rs`

## 功能

- 使用 Whisper 模型实时语音转文字
- 语音活动检测 (VAD) 实现自动识别
- OSC 集成，完美兼容 VRChat
- 多语言支持
- 通过图形界面轻松配置

## 系统要求

- 操作系统: Windows 或 Linux
- 音频: 正常工作的麦克风
- 内存: 最低 1GB (建议 4GB+，或显存用于加速模型)
- 存储: 1-2GB 模型文件空间

## 配置

在应用的设置界面中选择偏好模型、语言和音频设置。常用 Whisper 模型:
- `tiny.bin`: 最快，准确度较低
- `base.bin`: 良好平衡
- `medium.bin`: 推荐，准确度适中
- `large.bin`: 最高准确度，较慢

## 故障排除

没有音频? 检查麦克风权限和系统默认设备  
没有输出? 验证模型文件存在且为 ggml 格式  
OSC 无效? 确认 VRChat OSC 已启用且防火墙允许 9000 端口  

在应用的"日志"标签页查看详细的错误信息。

## 许可证

基于 GPL-3.0 许可证。

艺术资产使用 [CC BY-NC 4.0](CC-BY-NC-4.0) 协议提供。项目图标作者：清欢NP_C。

---

**注意**: 非官方 VRChat 工具。使用风险自负。