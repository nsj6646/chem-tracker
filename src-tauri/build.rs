fn main() {
    tauri_build::build();

    // Link Tauri's Windows Common Controls v6 manifest into test binaries too,
    // avoiding 0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND) in integration tests on Windows.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let res_lib = std::path::Path::new(&out_dir).join("resource.lib");
            if res_lib.exists() {
                println!("cargo:rustc-link-arg-tests={}", res_lib.display());
            }
        }
    }
}
