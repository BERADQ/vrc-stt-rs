fn main() {
    println!("cargo:rustc-env=CMAKE_CUDA_ARCHITECTURES=75;80;86;87;88;89;90;100;110;103;120;121");
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-env=CMAKE_GENERATOR=Ninja");
    }
}
