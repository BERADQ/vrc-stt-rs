# https://just.systems

default:
    cargo build -p backend
    RUST_LOG=info cargo run -p frontend

cuda:
    cargo build -p backend --features cuda --no-default-features
    RUST_LOG=info cargo run -p frontend --features cuda

release backend="":
    node release.js {{backend}}

get-model:
    wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin