# https://just.systems

default:
    cargo build -p backend
    RUST_LOG=info VRC_STT_BACKEND=./target/debug/backend cargo run -p frontend

release:
    node packaging.js

get-model:
    wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin