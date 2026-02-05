# https://just.systems

default:
    echo "Debug run on vulkan"
    cargo build -p backend
    RUST_LOG=info cargo run -p frontend

cuda:
    echo "Debug run on cuda"
    cargo build -p backend --features cuda --no-default-features
    RUST_LOG=info cargo run -p frontend --features cuda

openblas:
    echo "Debug run on openblas"
    cargo build -p backend --features openblas --no-default-features
    RUST_LOG=info cargo run -p frontend --features openblas

release backend="":
    node release.js {{ backend }}

get-model:
    wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O model/medium.bin
