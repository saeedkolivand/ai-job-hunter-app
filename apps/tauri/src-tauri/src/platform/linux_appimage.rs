//! Linux AppImage launch safeguards.
//!
//! On some Mesa/AMD Wayland setups (notably the Steam Deck) our AppImage bundles
//! its own `libwayland-client.so.0`, which clashes with the host Mesa EGL stack
//! and aborts the webview at init with:
//!
//! ```text
//! Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
//! ```
//!
//! leaving a blank window. The app only renders when **all three** mitigations
//! are applied:
//!
//! 1. `WEBKIT_DISABLE_DMABUF_RENDERER=1` — disable the WebKitGTK 2.42+ DMABUF
//!    renderer (read in-process at webview init; no re-exec needed).
//! 2. `WEBKIT_DISABLE_COMPOSITING_MODE=1` — disable accelerated compositing
//!    (likewise read in-process).
//! 3. Preload the **host** `libwayland-client.so.0` via `LD_PRELOAD` so the
//!    bundled copy is shadowed. `LD_PRELOAD` is consulted by the dynamic linker
//!    only at `exec()` time, so applying it requires re-exec'ing ourselves.
//!
//! This module is the **sole owner** of the env/process access these mitigations
//! need, keeping the binary launcher (`main.rs`) free of direct `std::env` /
//! re-exec calls (architecture rule R4: "env access only in platform/**").
//!
//! All of this is Linux-only; the entry point is a no-op on every other target.

/// Always set on Linux (idempotently) so the in-process WebKit env mitigations
/// land regardless of AppImage / Wayland detection. The user/dev may override
/// either by exporting their own value.
#[cfg(target_os = "linux")]
const WEBKIT_DMABUF_ENV: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
#[cfg(target_os = "linux")]
const WEBKIT_COMPOSITING_ENV: &str = "WEBKIT_DISABLE_COMPOSITING_MODE";

/// Re-exec guard: set just before the `LD_PRELOAD` re-exec so the freshly
/// exec'd process does not attempt the preload a second time (infinite loop).
#[cfg(target_os = "linux")]
const PRELOAD_ATTEMPTED_ENV: &str = "AJH_APPIMAGE_WAYLAND_PRELOAD_ATTEMPTED";

/// Apply the Linux/AppImage Wayland safeguards. MUST be called as the very first
/// statement of `main()` — before any WebKitGTK/GTK init and before the
/// native-host short-circuit — because (a) the WebKit env vars are read at
/// webview init and (b) the `LD_PRELOAD` re-exec must happen before the dynamic
/// linker has finished wiring up libraries.
///
/// On a successful preload re-exec this **replaces the current process image**
/// and never returns. In every other case (non-Linux, non-AppImage, X11 session,
/// no host libwayland found, already re-exec'd, or `exec()` failure) it returns
/// normally after applying whatever mitigations it could.
pub fn apply_wayland_appimage_safeguard() {
    #[cfg(target_os = "linux")]
    linux::apply();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{is_elf64, PRELOAD_ATTEMPTED_ENV, WEBKIT_COMPOSITING_ENV, WEBKIT_DMABUF_ENV};

    /// Host search roots for `libwayland-client.so.0`, most-specific first. These
    /// are fixed, trusted system directories — never anything derived from user
    /// input — so the resulting `LD_PRELOAD` entry is always an absolute path
    /// under a system library directory.
    const LIBWAYLAND_DIRS: &[&str] = &[
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib",
        "/lib64",
        "/lib",
    ];
    const LIBWAYLAND_SONAME: &str = "libwayland-client.so.0";

    pub(super) fn apply() {
        set_if_unset(WEBKIT_DMABUF_ENV, "1");
        set_if_unset(WEBKIT_COMPOSITING_ENV, "1");

        // The libwayland preload requires a re-exec; bail out of that part on any
        // condition where it would be wrong or looping. The WebKit vars above are
        // already applied regardless.
        if std::env::var_os(PRELOAD_ATTEMPTED_ENV).is_some() {
            // Already re-exec'd once — never loop.
            return;
        }
        if !is_appimage_launch() || !is_wayland_session() {
            // Dev build, extracted run, or X11 — the bundled-vs-host libwayland
            // clash does not apply, so skip the preload + re-exec.
            return;
        }
        let Some(libwayland) = find_host_libwayland() else {
            // No usable host libwayland — nothing to preload, don't re-exec.
            return;
        };

        // Prepend the absolute host path to any existing LD_PRELOAD (colon-sep).
        let preload = prepend_ld_preload(&libwayland, std::env::var_os("LD_PRELOAD"));
        // SAFETY: single-threaded, at the very top of main() before any other
        // thread (Tauri runtime, GTK) is spawned — no data race on the env.
        unsafe {
            std::env::set_var("LD_PRELOAD", &preload);
            std::env::set_var(PRELOAD_ATTEMPTED_ENV, "1");
        }

        // Re-exec self so the dynamic linker actually applies LD_PRELOAD. The
        // child inherits our env (the WebKit vars + LD_PRELOAD + guard we just
        // set), argv, and open fds (stdin/stdout/stderr), so the native-host
        // stdio path and all CLI args survive unchanged. `exec()` only returns on
        // error — log and continue so a failed re-exec never blocks the launch.
        let Ok(exe) = std::env::current_exe() else {
            // No self path to re-exec — continue without the preload.
            return;
        };
        let err = Command::new(exe).args(std::env::args_os().skip(1)).exec();
        log::warn!(
            "[startup] AppImage Wayland LD_PRELOAD re-exec failed, continuing without preload: {err}"
        );
    }

    /// Set an env var only when it is currently unset (respect a user override).
    fn set_if_unset(key: &str, value: &str) {
        if std::env::var_os(key).is_none() {
            // SAFETY: see `apply` — single-threaded top-of-main, no data race.
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }

    /// True when launched from an AppImage (the runtime exports `APPIMAGE` to the
    /// `.AppImage` path and `APPDIR` to the mounted root).
    fn is_appimage_launch() -> bool {
        std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some()
    }

    /// True on a Wayland session: either `WAYLAND_DISPLAY` is set or the session
    /// type is explicitly `wayland`.
    fn is_wayland_session() -> bool {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return true;
        }
        std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
    }

    /// First existing 64-bit-ELF `libwayland-client.so.0` across the trusted
    /// system roots, or `None`.
    fn find_host_libwayland() -> Option<PathBuf> {
        first_elf64_match(LIBWAYLAND_DIRS, LIBWAYLAND_SONAME)
    }

    pub(super) fn first_elf64_match(dirs: &[&str], soname: &str) -> Option<PathBuf> {
        dirs.iter()
            .map(|dir| Path::new(dir).join(soname))
            .find(|candidate| candidate.is_file() && is_elf64(candidate))
    }

    pub(super) fn prepend_ld_preload(
        path: &Path,
        existing: Option<std::ffi::OsString>,
    ) -> std::ffi::OsString {
        use std::ffi::OsString;
        let mut out = OsString::from(path);
        if let Some(existing) = existing {
            if !existing.is_empty() {
                out.push(":");
                out.push(existing);
            }
        }
        out
    }
}

/// Validate that `path` is a 64-bit ELF by reading its header, so a 32-bit lib
/// on a multilib host is rejected (preloading a 32-bit lib into our 64-bit
/// process would fail or corrupt the launch). Defined outside the `linux` module
/// so it is unit-testable on any host.
///
/// ELF header layout: bytes 0..4 are the magic `\x7FELF`; byte 4 (`EI_CLASS`) is
/// `2` for ELFCLASS64.
#[cfg(any(target_os = "linux", test))]
fn is_elf64(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 5];
    if f.read_exact(&mut header).is_err() {
        return false;
    }
    header[0..4] == [0x7f, b'E', b'L', b'F'] && header[4] == 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ajh-elf-test-{}-{}", std::process::id(), name));
        let mut f = std::fs::File::create(&dir).expect("create temp file");
        f.write_all(bytes).expect("write temp file");
        dir
    }

    #[test]
    fn elf64_accepts_64bit_elf_header() {
        // \x7FELF + EI_CLASS=2 (ELFCLASS64) + padding.
        let p = tmp_file("elf64", &[0x7f, b'E', b'L', b'F', 2, 0, 0, 0]);
        assert!(is_elf64(&p), "a 64-bit ELF header must be accepted");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn elf64_rejects_32bit_and_non_elf() {
        // EI_CLASS=1 (ELFCLASS32) → reject.
        let elf32 = tmp_file("elf32", &[0x7f, b'E', b'L', b'F', 1, 0, 0, 0]);
        assert!(!is_elf64(&elf32), "a 32-bit ELF must be rejected");
        let _ = std::fs::remove_file(&elf32);

        // Not an ELF at all (e.g. a text/ld script) → reject.
        let txt = tmp_file("txt", b"GROUP ( /usr/lib/libfoo.so )\n");
        assert!(!is_elf64(&txt), "a non-ELF file must be rejected");
        let _ = std::fs::remove_file(&txt);

        // Missing file → reject (no panic).
        assert!(!is_elf64(std::path::Path::new(
            "/nonexistent/libwayland-client.so.0"
        )));
    }

    // Linux-only: exercises the candidate-selection + LD_PRELOAD assembly without
    // re-exec'ing. These call into the `linux` submodule which only exists on Linux.
    #[cfg(target_os = "linux")]
    #[test]
    fn first_elf64_match_picks_first_existing_64bit_lib() {
        use std::path::Path;

        let base = std::env::temp_dir().join(format!("ajh-libwayland-{}", std::process::id()));
        let good = base.join("good");
        let bad = base.join("bad");
        std::fs::create_dir_all(&good).unwrap();
        std::fs::create_dir_all(&bad).unwrap();

        // `bad` holds a 32-bit ELF (must be skipped); `good` holds a 64-bit ELF.
        std::fs::write(
            bad.join("libwayland-client.so.0"),
            [0x7f, b'E', b'L', b'F', 1],
        )
        .unwrap();
        std::fs::write(
            good.join("libwayland-client.so.0"),
            [0x7f, b'E', b'L', b'F', 2],
        )
        .unwrap();

        let bad_s = bad.to_str().unwrap().to_string();
        let good_s = good.to_str().unwrap().to_string();

        // bad listed first → skipped (32-bit) → good chosen.
        let hit = super::linux::first_elf64_match(&[&bad_s, &good_s], "libwayland-client.so.0");
        assert_eq!(
            hit.as_deref(),
            Some(good.join("libwayland-client.so.0").as_path())
        );

        // None match an unknown soname.
        assert!(super::linux::first_elf64_match(&[&good_s], "libnope.so.0").is_none());

        let _ = std::fs::remove_dir_all(&base);

        // LD_PRELOAD assembly: prepend absolute path, preserve existing entries.
        let abs = Path::new("/usr/lib64/libwayland-client.so.0");
        assert_eq!(
            super::linux::prepend_ld_preload(abs, None),
            std::ffi::OsString::from("/usr/lib64/libwayland-client.so.0")
        );
        assert_eq!(
            super::linux::prepend_ld_preload(abs, Some("/x/y.so".into())),
            std::ffi::OsString::from("/usr/lib64/libwayland-client.so.0:/x/y.so")
        );
        assert_eq!(
            super::linux::prepend_ld_preload(abs, Some("".into())),
            std::ffi::OsString::from("/usr/lib64/libwayland-client.so.0")
        );
    }

    // Linux-only: `apply()` must RETURN (i.e. not re-exec) on the skip paths.
    // Re-exec replaces the process image, so any code after the `apply()` call
    // only runs when no re-exec was attempted — that observable return is the
    // assertion. We exercise the two skip branches that don't depend on the host
    // actually being an AppImage. All branches mutate process-global env, so they
    // live in one serialized test (parallel set/remove on these would race).
    #[cfg(target_os = "linux")]
    #[test]
    fn apply_skips_reexec_on_guard_and_non_appimage() {
        // Snapshot + clear the env we touch so the test is hermetic.
        let saved: Vec<(&str, Option<std::ffi::OsString>)> = [
            WEBKIT_DMABUF_ENV,
            WEBKIT_COMPOSITING_ENV,
            PRELOAD_ATTEMPTED_ENV,
            "APPIMAGE",
            "APPDIR",
            "WAYLAND_DISPLAY",
            "XDG_SESSION_TYPE",
        ]
        .iter()
        .map(|k| (*k, std::env::var_os(k)))
        .collect();
        let restore = || {
            for (k, v) in &saved {
                // SAFETY: single-threaded test.
                unsafe {
                    match v {
                        Some(val) => std::env::set_var(k, val),
                        None => std::env::remove_var(k),
                    }
                }
            }
        };
        // SAFETY: single-threaded test mutating process-global env.
        unsafe {
            for k in [
                WEBKIT_DMABUF_ENV,
                WEBKIT_COMPOSITING_ENV,
                PRELOAD_ATTEMPTED_ENV,
                "APPIMAGE",
                "APPDIR",
                "WAYLAND_DISPLAY",
                "XDG_SESSION_TYPE",
            ] {
                std::env::remove_var(k);
            }
        }

        // Branch A: re-exec guard already set → must return without re-exec'ing,
        // even if it otherwise looks like an AppImage Wayland launch.
        unsafe {
            std::env::set_var(PRELOAD_ATTEMPTED_ENV, "1");
            std::env::set_var("APPIMAGE", "/tmp/App.AppImage");
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        }
        super::linux::apply(); // returns here ⇒ no re-exec occurred
                               // The WebKit vars are still applied on every Linux run.
        assert_eq!(std::env::var(WEBKIT_DMABUF_ENV).ok().as_deref(), Some("1"));
        assert_eq!(
            std::env::var(WEBKIT_COMPOSITING_ENV).ok().as_deref(),
            Some("1")
        );

        // Branch B: not an AppImage and X11 session → must return without re-exec.
        unsafe {
            std::env::remove_var(PRELOAD_ATTEMPTED_ENV);
            std::env::remove_var(WEBKIT_DMABUF_ENV);
            std::env::remove_var(WEBKIT_COMPOSITING_ENV);
            std::env::remove_var("APPIMAGE");
            std::env::remove_var("APPDIR");
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::set_var("XDG_SESSION_TYPE", "x11");
        }
        super::linux::apply(); // returns here ⇒ no re-exec
                               // WebKit vars still applied; guard NOT set (no re-exec was attempted).
        assert_eq!(std::env::var(WEBKIT_DMABUF_ENV).ok().as_deref(), Some("1"));
        assert!(std::env::var_os(PRELOAD_ATTEMPTED_ENV).is_none());

        restore();
    }
}
