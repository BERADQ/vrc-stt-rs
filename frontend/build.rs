fn main() {
    if cfg!(target_os = "windows") {
        winres::WindowsResource::new()
            .set_icon("../arts/vrc-stt-rs.ico")
            .compile().unwrap();
    }
}