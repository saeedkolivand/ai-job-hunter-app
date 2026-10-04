use super::*;
use std::io::Write;

fn tmp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ajh-elf-test-{}-{}", std::process::id(), name));
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

// Linux-only: the safeguard's exec-free decisions. We assert `plan_from_env`
// (the scoping verdict) and `apply_webkit_mitigations` (the env it sets)
// DIRECTLY — never the full `apply()`, which can re-`exec()` the test binary
// when the host has a real libwayland (which CI runners do) and would hang
// the test process. The exec-capable tail of `apply()` is not unit-testable
// in-process and is deliberately left uncovered here.
//
// Mutates process-global env (APPIMAGE / WAYLAND_DISPLAY / XDG_SESSION_TYPE /
// the guard / the WebKit vars), so serialized against the other env-mutating
// test to avoid races under threaded `cargo test`.
#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn plan_and_webkit_mitigations_are_scoped_to_appimage_wayland() {
    use super::linux::{apply_webkit_mitigations, plan_from_env, Plan};

    // Keys this test reads/writes, so it can snapshot + clear them hermetically.
    const KEYS: &[&str] = &[
        WEBKIT_DMABUF_ENV,
        WEBKIT_COMPOSITING_ENV,
        PRELOAD_ATTEMPTED_ENV,
        "APPIMAGE",
        "APPDIR",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
    ];
    let saved: Vec<(&str, Option<std::ffi::OsString>)> =
        KEYS.iter().map(|k| (*k, std::env::var_os(k))).collect();
    let clear_all = || {
        // SAFETY: single-threaded test mutating process-global env.
        unsafe {
            for k in KEYS {
                std::env::remove_var(k);
            }
        }
    };
    clear_all();

    // Case A: re-exec guard already set → Skip, even if it otherwise looks
    // like an AppImage Wayland launch (a prior process already applied the
    // mitigations and we inherited them; re-applying would loop).
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var(PRELOAD_ATTEMPTED_ENV, "1");
        std::env::set_var("APPIMAGE", "/tmp/App.AppImage");
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
    }
    assert_eq!(plan_from_env(), Plan::Skip, "guard set ⇒ Skip");
    clear_all();

    // Case B: not an AppImage and X11 session → Skip (Finding 1: no needless
    // DMABUF/compositing regression on a healthy non-AppImage / X11 / dev
    // launch). `apply_webkit_mitigations` is NOT called on this path.
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var("XDG_SESSION_TYPE", "x11");
    }
    assert_eq!(plan_from_env(), Plan::Skip, "non-AppImage / X11 ⇒ Skip");
    assert!(
        std::env::var_os(WEBKIT_DMABUF_ENV).is_none(),
        "Skip path must NOT set WEBKIT_DISABLE_DMABUF_RENDERER"
    );
    assert!(std::env::var_os(WEBKIT_COMPOSITING_ENV).is_none());
    clear_all();

    // Case C: AppImage + Wayland → ApplyAndPreload, and the in-process WebKit
    // mitigations land (Finding 1: applied in the scenario they target). We
    // call `apply_webkit_mitigations` directly — NOT `apply()` — so no
    // libwayland search and no re-exec can ever occur from this test.
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var("APPIMAGE", "/tmp/App.AppImage");
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
    }
    assert_eq!(
        plan_from_env(),
        Plan::ApplyAndPreload,
        "AppImage + Wayland ⇒ ApplyAndPreload"
    );
    apply_webkit_mitigations();
    assert_eq!(
        std::env::var(WEBKIT_DMABUF_ENV).ok().as_deref(),
        Some("1"),
        "AppImage + Wayland must set WEBKIT_DISABLE_DMABUF_RENDERER"
    );
    assert_eq!(
        std::env::var(WEBKIT_COMPOSITING_ENV).ok().as_deref(),
        Some("1")
    );

    // `apply_webkit_mitigations` respects a user override (set_if_unset).
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var(WEBKIT_DMABUF_ENV, "user-value");
    }
    apply_webkit_mitigations();
    assert_eq!(
        std::env::var(WEBKIT_DMABUF_ENV).ok().as_deref(),
        Some("user-value"),
        "must not clobber a user-exported WebKit value"
    );

    // Restore the caller's env.
    // SAFETY: single-threaded test.
    unsafe {
        for (k, v) in &saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}

// Linux-only: Finding 2 — when a re-exec cannot happen the env mutation
// (prepended LD_PRELOAD + guard) is rolled back so the still-running process
// and its children don't inherit a bogus preload. `restore_preload_env` is
// the unit that does that rollback on both failure paths (`current_exe()`
// err and `exec()` return); the real `exec()` can't be exercised in-process
// without replacing it, so we assert the rollback unit directly.
//
// Mutates process-global env (LD_PRELOAD + the guard), so serialized against
// the other env-mutating test to avoid races under threaded `cargo test`.
#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn restore_preload_env_rolls_back_mutation() {
    const KEYS: &[&str] = &["LD_PRELOAD", PRELOAD_ATTEMPTED_ENV];
    let saved: Vec<(&str, Option<std::ffi::OsString>)> =
        KEYS.iter().map(|k| (*k, std::env::var_os(k))).collect();

    // Case 1: there WAS a prior LD_PRELOAD — restore the exact prior value
    // and clear the guard (drop the prepended host-lib entry we added).
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var("LD_PRELOAD", "/usr/lib64/libwayland-client.so.0:/x/y.so");
        std::env::set_var(PRELOAD_ATTEMPTED_ENV, "1");
    }
    super::linux::restore_preload_env(Some(std::ffi::OsStr::new("/x/y.so")));
    assert_eq!(
        std::env::var("LD_PRELOAD").ok().as_deref(),
        Some("/x/y.so"),
        "must restore the exact prior LD_PRELOAD"
    );
    assert!(
        std::env::var_os(PRELOAD_ATTEMPTED_ENV).is_none(),
        "must clear the re-exec guard on rollback"
    );

    // Case 2: there was NO prior LD_PRELOAD — remove ours entirely.
    // SAFETY: single-threaded test.
    unsafe {
        std::env::set_var("LD_PRELOAD", "/usr/lib64/libwayland-client.so.0");
        std::env::set_var(PRELOAD_ATTEMPTED_ENV, "1");
    }
    super::linux::restore_preload_env(None);
    assert!(
        std::env::var_os("LD_PRELOAD").is_none(),
        "must remove LD_PRELOAD when there was no prior value"
    );
    assert!(std::env::var_os(PRELOAD_ATTEMPTED_ENV).is_none());

    // Restore the caller's env.
    // SAFETY: single-threaded test.
    unsafe {
        for (k, v) in &saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}
