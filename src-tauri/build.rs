fn main() {
    tauri_build::build();

    #[cfg(target_os = "macos")]
    build_system_audio_shim();

    #[cfg(target_os = "macos")]
    build_permissions_shim();
}

/// Compiles `native/system_audio_shim.m` — the ScreenCaptureKit build-time
/// Objective-C shim (see src/capture/system_audio.rs) — and links the
/// frameworks it calls into. This is the "minimal build-time Objective-C
/// shim" path named in docs/mutter-project-plan.md Section 9/15, chosen
/// over an objc2-based crate bridge: ScreenCaptureKit's surface used here
/// (SCStream/SCStreamConfiguration/SCContentFilter, async completion
/// handlers, an AudioBufferList extraction) is small and stable enough that
/// a few hundred lines of plain Objective-C behind a 2-function C ABI is
/// less risk than pulling in a large generated binding crate for it.
#[cfg(target_os = "macos")]
fn build_system_audio_shim() {
    println!("cargo:rerun-if-changed=native/system_audio_shim.m");
    println!("cargo:rerun-if-changed=native/system_audio_shim.h");

    cc::Build::new()
        .file("native/system_audio_shim.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .compile("mutter_system_audio_shim");

    for framework in ["ScreenCaptureKit", "CoreMedia", "CoreAudio", "Foundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

/// Compiles `native/permissions_shim.m` — the one-function Objective-C shim
/// for microphone authorization status (see src/permissions.rs). Not
/// merged into the ScreenCaptureKit shim above — different concern,
/// different framework (AVFoundation), and the AX/Screen-Recording checks
/// permissions.rs also needs have plain C entry points declared directly in
/// Rust, so this file's only job is the one thing that genuinely needs
/// Objective-C.
#[cfg(target_os = "macos")]
fn build_permissions_shim() {
    println!("cargo:rerun-if-changed=native/permissions_shim.m");
    println!("cargo:rerun-if-changed=native/permissions_shim.h");

    cc::Build::new()
        .file("native/permissions_shim.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .compile("mutter_permissions_shim");

    println!("cargo:rustc-link-lib=framework=AVFoundation");
}
