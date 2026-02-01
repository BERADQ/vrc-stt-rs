# VRC-STT-RS

[English Version](./README.md)

一个极简、高性能的 VRChat 语音转文本（STT）软件，使用 Rust 编写。该工具捕获您的麦克风输入，使用 OpenAI 的 Whisper 模型实时转录语音，并通过 OSC（开放声音控制）将转录文本发送到 VRChat。

## 功能特性

- **语音活动检测（VAD）**：使用 WebRTC 的 VAD 算法，支持可配置阈值
- **语音转录**：使用 OpenAI 的 Whisper 模型
- **OSC 集成**：与 VRChat 无缝通信
- **可配置参数**：通过 JSON 配置文件进行配置
- **多语言支持**：通过设置配置支持多种语言

## 前提条件

### 系统要求
- **操作系统**：Windows、Linux
- **音频**：可用的麦克风输入
- **内存**：最低 4GB，建议 8GB+（用于较大的 Whisper 模型）
- **存储空间**：约 1-2GB 用于 Whisper 模型文件

## 构建与安装

### 1. 克隆仓库
```bash
git clone https://github.com/BERADQ/vrc-stt-rs.git
cd vrc-stt-rs
```

### 2. 构建
```bash
cargo build --release
```

#### 3.1 或下载预构建二进制文件
从 [GitHub Releases](https://github.com/BERADQ/vrc-stt-rs/releases) 下载预构建二进制文件，并将其放置在任何目录中。

### 3. 下载 Whisper 模型
从 [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp) 或其他来源下载 Whisper 模型文件，并将其放置在 `model/` 目录中：

```bash
# 示例：下载中等模型（推荐，平衡速度与准确性）
wget -P model/ https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
```

可用模型（大小/性能权衡）：
- `tiny.bin` - 最快，准确性最低
- `base.bin` - 大多数用户的最佳平衡
- `small.bin` - 更好的准确性
- `medium.bin` - 推荐，准确性良好
- `large.bin` - 最佳准确性，最慢

## 配置

编辑 `config.json` 以自定义行为：

```json
{
  // Whisper 模型文件路径
  "model_path": "./model/medium.bin",
  // 语音活动检测（VAD）设置
  "vad": {
    "threshold_level": 0.05,
    "debounce_times": 32
  },
  // 更改转录语言代码
  "language": "zh",
  // Whisper 的初始提示
  "initial_prompt": "使用简体中文输出",
  "udp": {
    "port": 5005,
    // 发送到 VRChat 的 OSC 地址
    "to": "127.0.0.1:9000"
  }
}
```

### 环境变量
- `CONFIG_PATH`：覆盖默认配置文件位置

## 使用说明

### 1. 配置 VRChat
确保 VRChat 运行并启用了 OSC：
1. 进入 **设置** → **OSC** → **启用**
2. 记下 OSC 端口（默认：`9000`）

### 2. 运行应用程序
```bash
# 开发模式（带调试）
cargo run

# 发布模式（优化）
cargo run --release

# 使用自定义配置文件路径
CONFIG_PATH=./myconfig.json cargo run --release
```

#### 2.1 运行预构建二进制文件
```bash
# 使用默认配置文件路径
./vrc-stt-rs

# 使用自定义配置文件路径
CONFIG_PATH=./myconfig.json ./vrc-stt-rs
```

### 3. 使用软件
运行后，软件将：
1. 监控麦克风是否有语音
2. 显示开始/停止录音的消息
3. 转录语音并发送到 VRChat
4. 将转录结果打印到控制台

示例输出：
```
Start recording
Recording ended, audio length 5120 samples, transcribing...
Starting transcription, audio length 5120 samples
Transcription complete: Hello VRChat!
```

### 4. 停止应用程序
按 `Ctrl+C` 停止应用程序。

## 故障排除

### 常见问题

#### 无音频输入
- 检查系统设置中的默认麦克风
- 验证应用程序是否有麦克风权限
- 使用 `arecord`（Linux）或系统录音工具进行测试

#### 无转录输出
- 确认 Whisper 模型文件存在于配置路径中
- 检查模型文件是否兼容（ggml 格式）
- 确保有足够的内存加载模型

#### OSC 无法到达 VRChat
- 确认 VRChat 设置中已启用 OSC
- 检查防火墙是否允许端口 9000 上的 UDP 流量
- 验证配置中的 IP 地址是否与系统匹配

#### CPU 使用率过高
- 使用较小的 Whisper 模型（tiny/base 而不是 medium/large）
- 考虑使用 CUDA/ROCm 或其他硬件加速的 GPU 加速
- 调高 VAD debounce_times 值

### 调试
通过开发模式启用详细日志：
```bash
RUST_LOG=debug cargo run
```

检查音频设备配置：
```bash
# 列出音频设备
cargo run --features=cpal/debug
```

## 性能调优

### 提高准确性
- 使用较大的 Whisper 模型（medium/large）
- 降低 VAD threshold_level（例如 0.02）
- 增加 debounce_times（例如 64）

### 降低延迟
- 使用较小的 Whisper 模型（tiny/base）
- 提高 VAD threshold_level（例如 0.1）
- 减少 debounce_times（例如 16）

## 开发

### 从源代码构建
```bash
# 调试构建
cargo build
```

### 添加功能
1. 在 `config.rs` 和 `config.json` 中添加新的配置选项
2. 在适当的模块中实现功能
3. 如果需要，更新 `Cargo.toml` 以添加新的依赖项
4. 部署前进行彻底测试

## 许可证

本项目采用 MIT 许可证 - 有关详细信息，请参阅 LICENSE 文件。

## 致谢

- [OpenAI Whisper](https://github.com/openai/whisper) 提供语音识别模型
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) 提供 C++ 实现
- [whisper-rs](https://github.com/tazz4843/whisper-rs) 提供 Rust 绑定
- [WebRTC VAD](https://github.com/daily-co/webrtc-vad) 提供语音活动检测
- [rosc](https://github.com/keschwa/rosc) 提供 OSC 实现
- [cpal](https://github.com/RustAudio/cpal) 提供跨平台音频 I/O

## 支持

如果遇到问题：
1. 查看[故障排除](#故障排除)部分
2. 搜索现有问题
3. 创建新问题时请提供：
   - 您的配置
   - 错误信息
   - 重现步骤
   - 系统信息

## 路线图

- [ ] 多麦克风支持
- [ ] 自定义热键用于手动控制

---

**注意**：本软件与 VRChat Inc. 无关，也未获得其认可。使用时风险自负。