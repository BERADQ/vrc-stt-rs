# https://just.systems

default:
    cargo build -p backend
    RUST_LOG=info VRC_STT_BACKEND=./target/debug/backend cargo run -p frontend

release:
    node packaging.js