fn main() {
    // Windows/MSVC only: keep `cargo test` from aborting at load with
    // STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139).
    //
    // The lib statically imports `comctl32.dll!TaskDialogIndirect` (through the
    // wry/muda/rfd dialog path). That export lives only in ComCtl32 v6 (WinSxS),
    // which the loader binds solely when the process manifest declares
    // `Microsoft.Windows.Common-Controls 6.0.0.0`. tauri-build embeds that manifest
    // into bin targets, and integration `[[test]]` targets can take a manifest via
    // `rustc-link-arg-tests` — but the library's own unit-test harness (`--lib`,
    // where ~all of our tests live) is a *lib* target: Cargo has no link-arg
    // category that reaches it except the crate-wide `rustc-link-arg`, and adding
    // `/MANIFEST:EMBED` there collides with tauri-build's bin manifest resource
    // (CVT1100 duplicate MANIFEST). So instead of manifesting the harness, delay-load
    // comctl32: the v6-only import is no longer bound at process start (it resolves on
    // first call, which the tests never make). Bins/dialogs still call it lazily and
    // resolve v6 via tauri-build's manifest, so runtime behavior is unchanged.
    //
    // Gate on the *target* env the build script sees at runtime, NOT `#[cfg(windows)]`
    // (a `cfg` in a build script reflects the host, not the compile target).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        // `/DELAYLOAD` needs the delay-load helper (`__delayLoadHelper2`).
        println!("cargo:rustc-link-lib=dylib=delayimp");
    }

    tauri_build::build()
}
