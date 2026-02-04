# VRC-STT-RS

<img src="./arts/vrc-stt-rs.svg" width="256">

Whisperを使用したVRChat向けリアルタイム音声認識。マイク入力をキャプチャし、OpenAIのWhisperモデルで音声をテキストに変換して、OSC経由でVRChatに送信します。

[English Version](./README.md)

## クイックスタート

1. [最新リリース](https://github.com/BERADQ/vrc-stt-rs/releases)をダウンロード
2. Whisperモデルを取得:
   ```bash
   mkdir model
   wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
   ```
3. VRChatでOSCを有効化: `設定 → OSC → 有効`
4. 実行: `./vrc-stt-rs`

## 機能

- Whisperモデルによるリアルタイム音声テキスト変換
- 音声検出(VAD)による自動音声認識
- VRChatとのシームレスな互換性のためのOSC統合
- 多言語対応
- GUI設定による調整可能

## 動作条件

- OS: WindowsまたはLinux
- 音声: 作動するマイク
- メモリ: 最低1GB (4GB以上推奨、又は高速モデル用GPUメモリ)
- ストレージ: モデル用に1-2GB

## 設定

アプリの設定タブで希望モデル、言語、オーディオ設定を選択してください。一般的なWhisperモデル:
- `tiny.bin`: 最も高速、精度低め
- `base.bin`: 良好なバランス
- `medium.bin`: 精度がおすすめ
- `large.bin`: 最高精度、低速

## ロードマップ

- [ ] プッシュトゥートーク対応
- [ ] オプションの自動モデルダウンロード
- [ ] 録音一時停止ボタン

## トラブルシューティング

音声なし？ マイク許可とシステムデフォルトデバイスを確認  
出力なし？ モデルファイルが存在しggml形式であることを確認  
OSCが動作しない？ VRChat OSCが有効かつファイアウォールがポート9000を許可していることを確認  

詳細なエラーメッセージはアプリ内のログタブを確認してください。

## ライセンス

GPL-3.0ライセンス。

アートアセットは[CC BY-NC 4.0](CC-BY-NC-4.0)ライセンスで提供されています。プロジェクトアイコンは清欢NP_Cによるものです。

---

**注意**: 非公式VRChatツール。自己責任で使用してください。