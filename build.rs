fn main() {
    // NOTE: this build script is compiled for the *host*, so `cfg!(target_os)`
    // here would test macOS even when cross-compiling to Windows. Cargo exposes
    // the actual target platform to build scripts via CARGO_CFG_TARGET_OS.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=CoreLocation");
    }
}
