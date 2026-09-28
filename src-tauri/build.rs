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
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // tauri-winres embeds its default manifest only into binary targets.
        // Library/integration test executables also import TaskDialogIndirect,
        // which requires Common Controls v6 before the process can start.
        // Generate the manifest at link time for every target, and keep Tauri's
        // icon/version resources without a second, conflicting manifest.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        tauri_build::try_build(attributes).expect("failed to build Windows resources");
    } else {
        tauri_build::build()
    }
}
