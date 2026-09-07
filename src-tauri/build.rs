fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let library = std::env::var("ZVEC_LIB_DIR")
        .unwrap_or_else(|_| root.join("runtime/zvec").display().to_string());
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Resources/runtime/zvec");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{library}");
        }
        Ok("linux") => {
            println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/Casy/runtime/zvec");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{library}");
        }
        _ => {}
    }
    tauri_build::build()
}
