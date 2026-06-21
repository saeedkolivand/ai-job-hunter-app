//! Native splash window — instant, theme-aware cold-start cover.
//!
//! A small, undecorated, always-on-top splash window is created in the Tauri
//! `setup` hook and shown immediately while the (hidden) main window loads its
//! webview and the React app mounts. Once the renderer signals readiness (the
//! `app_ready` command) — or a safety timeout fires — the splash is closed and
//! the main window is revealed (`show()` + `set_focus()`).
//!
//! ## Theme
//! The splash is themed light/dark to match the user's last *effective* color
//! scheme. The renderer writes that scheme to a tiny "theme mirror" file (see
//! [`THEME_MIRROR_FILE`] / [`read_theme_mirror`]); on first launch (file absent)
//! we fall back to the OS theme reported by Tauri's window `theme()`. The
//! resolved value rides the splash URL as `?theme=light|dark`, read by an inline
//! `<script>` in `splash.html` before first paint.
//!
//! ## Reveal coordination (the careful part)
//! * A `~700ms` minimum splash display is enforced so a fast cold start does not
//!   flash the splash for a single frame.
//! * The reveal is **idempotent** — a [`RevealGuard`] (`AtomicBool`) ensures that
//!   duplicate `app_ready` calls AND the safety timeout can never double-reveal.
//! * A `~10s` safety timeout force-reveals (via the same guarded path) if
//!   `app_ready` never arrives, so a hung renderer can never trap the user behind
//!   a hidden main window.
//!
//! ## Async-from-setup safety
//! Any async work scheduled from `setup` MUST use [`tauri::async_runtime::spawn`]
//! — never a bare `tokio::spawn`. There is no ambient Tokio reactor during
//! `setup`, so a bare spawn panics at boot. See lesson
//! `reference_tauri_setup_spawn_runtime`. The smoke test in this module exercises
//! the real `tauri::async_runtime` path (not `#[tokio::test]`) to catch a
//! regression here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};
use crate::observability::Span;

/// Label of the programmatically-created splash window.
const SPLASH_LABEL: &str = "splash";

/// Label of the main application window (declared in `tauri.conf.json`).
const MAIN_LABEL: &str = "main";

/// Bundled splash asset, served from the frontend dist root (Vite copies
/// `apps/tauri/public/` verbatim into `dist/`). The `?theme=` query is appended
/// by [`spawn`].
const SPLASH_ASSET: &str = "splash.html";

/// Path of the theme-mirror file under the app data dir. Its content is exactly
/// the literal `"light"` or `"dark"` — the renderer writes its *effective*
/// displayed color scheme here (a separate frontend task). The two literals are
/// the only valid contents; anything else is treated as absent.
pub const THEME_MIRROR_FILE: &str = "ui-theme";

/// Minimum time the splash stays visible, so a fast cold start does not flash it
/// for a single frame.
const MIN_SPLASH: Duration = Duration::from_millis(700);

/// Safety net: force-reveal the main window after this long even if `app_ready`
/// never arrives (a hung renderer must never trap the user behind a hidden main
/// window).
const SAFETY_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolved splash color scheme. Maps 1:1 to the renderer's *effective* scheme
/// and to `tauri::Theme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashTheme {
    Light,
    Dark,
}

impl SplashTheme {
    /// The `?theme=` query value read by the inline script in `splash.html`.
    pub fn as_query_value(self) -> &'static str {
        match self {
            SplashTheme::Light => "light",
            SplashTheme::Dark => "dark",
        }
    }
}

impl From<tauri::Theme> for SplashTheme {
    fn from(t: tauri::Theme) -> Self {
        // `tauri::Theme` is `#[non_exhaustive]`; any non-`Dark` (incl. future
        // variants) resolves to light — the safe, high-contrast default.
        match t {
            tauri::Theme::Dark => SplashTheme::Dark,
            _ => SplashTheme::Light,
        }
    }
}

/// Pure theme resolver — the single decision point, kept `AppHandle`-free so it
/// is fully unit-testable.
///
/// * `mirror` — raw contents of the theme-mirror file (`None` if absent/unreadable).
/// * `os_theme` — the OS theme to fall back to when the mirror is missing or not
///   one of the two valid literals.
///
/// The mirror is authoritative when it is exactly `"light"` or `"dark"` (after
/// trimming surrounding whitespace/newlines the renderer might write). Any other
/// content — empty, garbage, a stray BOM — falls through to the OS theme.
pub fn resolve_theme(mirror: Option<&str>, os_theme: SplashTheme) -> SplashTheme {
    match mirror.map(str::trim) {
        Some("light") => SplashTheme::Light,
        Some("dark") => SplashTheme::Dark,
        _ => os_theme,
    }
}

/// The theme-mirror file path inside `dir`. The SINGLE place the filename is
/// joined onto a directory, so the reader and writer can never target different
/// files. Callers pass the app data dir (from `platform::config::data_dir()`),
/// keeping that env-backed knowledge in `platform`.
fn theme_mirror_path_in(dir: &Path) -> PathBuf {
    dir.join(THEME_MIRROR_FILE)
}

/// Read the theme-mirror file from the app data dir, returning its raw contents
/// if present and readable. The path/dir knowledge lives in `platform::config`,
/// honoring the centralized-config rule. (Trimming/validation is the caller's —
/// see [`resolve_theme`].)
fn read_theme_mirror() -> Option<String> {
    read_theme_mirror_from(&crate::platform::config::data_dir())
}

/// Path-explicit reader — reads `<dir>/ui-theme`. Shares [`theme_mirror_path_in`]
/// with the writer so a round-trip is guaranteed to hit the same file. The
/// `data_dir()`-backed [`read_theme_mirror`] is the production wrapper.
fn read_theme_mirror_from(dir: &Path) -> Option<String> {
    std::fs::read_to_string(theme_mirror_path_in(dir)).ok()
}

/// Validate a renderer-supplied scheme down to the closed `light`|`dark` set.
///
/// The renderer is the only writer of the mirror, so the value is treated as
/// untrusted input at the IPC boundary: trimmed, then accepted ONLY if it is
/// exactly one of the two literals the reader understands. Anything else is a
/// [`AppError::Validation`] — the writer never persists out-of-set content, so
/// the file can only ever hold `light` or `dark` (matching [`resolve_theme`]'s
/// contract). The filename is fixed (never renderer-controlled).
///
/// `pub(crate)` so `commands::system` tests can assert the IPC rejection path
/// directly without constructing an `AppHandle` or touching the filesystem.
pub(crate) fn validate_scheme(scheme: &str) -> AppResult<&'static str> {
    match scheme.trim() {
        "light" => Ok("light"),
        "dark" => Ok("dark"),
        other => Err(AppError::Validation(format!(
            "set_theme_mirror: scheme must be \"light\" or \"dark\", got {other:?}"
        ))),
    }
}

/// Persist the renderer's effective color scheme to the theme-mirror file so the
/// next cold start themes the native splash before first paint. Validates the
/// scheme to the closed `light`|`dark` set (rejecting anything else WITHOUT
/// writing) and shares the path with the reader via [`theme_mirror_path_in`].
///
/// The path/dir knowledge stays in `platform::config`, honoring the
/// centralized-config rule.
pub fn write_theme_mirror(scheme: &str) -> AppResult<()> {
    write_theme_mirror_in(&crate::platform::config::data_dir(), scheme)
}

/// Path-explicit writer — validates then writes `<dir>/ui-theme`. On an invalid
/// scheme it returns `Err` BEFORE any filesystem touch, so a rejected call never
/// creates or modifies the file. The `data_dir()`-backed [`write_theme_mirror`]
/// is the production wrapper.
fn write_theme_mirror_in(dir: &Path, scheme: &str) -> AppResult<()> {
    let span = Span::begin("splash", "write_theme_mirror".to_string());
    let result: AppResult<()> = (|| {
        let value = validate_scheme(scheme)?;
        std::fs::create_dir_all(dir)?;
        std::fs::write(theme_mirror_path_in(dir), value)?;
        Ok(())
    })();
    match &result {
        Ok(()) => span.end(true),
        Err(e) => span.end_with(&e.to_string(), false),
    }
    result
}

/// Best-effort OS theme via the (hidden) main window's reported theme. Defaults
/// to dark on any read failure — the app's brand canvas is dark, so dark is the
/// least-jarring fallback for a first paint.
fn detect_os_theme(app: &AppHandle) -> SplashTheme {
    app.get_webview_window(MAIN_LABEL)
        .and_then(|w| w.theme().ok())
        .map(SplashTheme::from)
        .unwrap_or(SplashTheme::Dark)
}

/// Resolve the splash theme for this launch: mirror file first, OS theme as the
/// first-launch fallback.
fn resolve_for_launch(app: &AppHandle) -> SplashTheme {
    resolve_theme(read_theme_mirror().as_deref(), detect_os_theme(app))
}

/// Idempotent reveal guard. Wraps an `AtomicBool` so the first caller to
/// [`RevealGuard::claim`] wins and every later caller (a duplicate `app_ready`,
/// the safety timeout) is a no-op. Managed in Tauri state by [`spawn`].
#[derive(Default)]
pub struct RevealGuard {
    revealed: AtomicBool,
}

impl RevealGuard {
    /// Atomically claim the single reveal. Returns `true` exactly once — for the
    /// first caller — and `false` for every subsequent call.
    pub fn claim(&self) -> bool {
        // `swap` is the compare-and-set primitive we need: set to `true` and
        // return the PREVIOUS value. Previous `false` ⇒ we won the race.
        !self.revealed.swap(true, Ordering::SeqCst)
    }
}

/// Close the splash and reveal the main window — exactly once.
///
/// Idempotent via the managed [`RevealGuard`]: the first call (whether from
/// `app_ready` or the safety timeout) performs the work; all later calls return
/// immediately. Safe to call from any thread / async task.
pub fn reveal_main(app: &AppHandle) {
    let guard = app.state::<Arc<RevealGuard>>();
    if !guard.claim() {
        return; // Already revealed — duplicate call (or the timeout raced).
    }

    // Span only the winning reveal — the no-op duplicate/timeout calls return above
    // without logging, so the span fires exactly once per launch.
    let span = Span::begin("splash", "reveal_main".to_string());
    if let Some(splash) = app.get_webview_window(SPLASH_LABEL) {
        let _ = splash.close();
    }
    if let Some(main) = app.get_webview_window(MAIN_LABEL) {
        let _ = main.show();
        let _ = main.set_focus();
    }
    span.end(true);
}

/// Create the splash window (themed via `?theme=`), manage the [`RevealGuard`],
/// and schedule the safety timeout. Called once from the Tauri `setup` hook.
///
/// Returns the [`Instant`] the splash was shown so [`crate::commands::system::app_ready`]
/// can enforce [`MIN_SPLASH`]. On a splash-creation failure the main window is
/// revealed immediately (degraded but never trapped) and `None` is returned.
///
/// CRITICAL: the safety-timeout task is spawned with [`tauri::async_runtime::spawn`]
/// — a bare `tokio::spawn` here would panic (no reactor during `setup`).
pub fn spawn(app: &AppHandle) -> Option<Instant> {
    app.manage(Arc::new(RevealGuard::default()));

    let theme = resolve_for_launch(app);
    let span = Span::begin("splash", format!("spawn theme={}", theme.as_query_value()));
    let url = format!("{SPLASH_ASSET}?theme={}", theme.as_query_value());

    let builder = WebviewWindowBuilder::new(app, SPLASH_LABEL, WebviewUrl::App(url.into()))
        .title("AI Job Hunter")
        .inner_size(440.0, 280.0)
        .center()
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .background_color(theme_background(theme));

    match builder.build() {
        Ok(_) => {
            let shown_at = Instant::now();
            schedule_safety_reveal(app);
            span.end(true);
            Some(shown_at)
        }
        Err(e) => {
            // Splash failed to create — never trap the user behind a hidden main
            // window. Reveal immediately (no minimum-display delay).
            log::warn!("[splash] failed to create splash window (revealing main): {e}");
            span.end_with(&e.to_string(), false);
            reveal_main(app);
            None
        }
    }
}

/// Splash window chrome background, matched to the resolved theme so the native
/// frame paints brand-correct before the webview's own paint lands.
fn theme_background(theme: SplashTheme) -> tauri::window::Color {
    match theme {
        // `#0a0a14` — the dark brand canvas (mirrors tauri.conf main window).
        SplashTheme::Dark => tauri::window::Color(0x0a, 0x0a, 0x14, 0xff),
        // Paper white for the light variant.
        SplashTheme::Light => tauri::window::Color(0xf8, 0xf8, 0xfb, 0xff),
    }
}

/// Schedule the force-reveal safety net. Uses `tauri::async_runtime::spawn` (NOT
/// `tokio::spawn`) so it is valid when scheduled from `setup` (no ambient
/// reactor). The reveal goes through the guarded [`reveal_main`], so if
/// `app_ready` already revealed, this is a no-op.
fn schedule_safety_reveal(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let span = Span::begin("splash", "safety_reveal_wait".to_string());
        tokio::time::sleep(SAFETY_TIMEOUT).await;
        span.end(true);
        reveal_main(&handle);
    });
}

/// Reveal after honoring the [`MIN_SPLASH`] minimum-display window.
///
/// Given the [`Instant`] the splash was shown, sleeps the remainder of
/// [`MIN_SPLASH`] (if any) before revealing. Spawned by the `app_ready` command
/// so the command itself returns immediately. Goes through guarded
/// [`reveal_main`], so it is idempotent with the safety timeout and any duplicate
/// `app_ready`.
pub fn reveal_after_min_display(app: &AppHandle, shown_at: Instant) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let span = Span::begin("splash", "reveal_after_min_display".to_string());
        let elapsed = shown_at.elapsed();
        if let Some(remaining) = MIN_SPLASH.checked_sub(elapsed) {
            tokio::time::sleep(remaining).await;
        }
        span.end(true);
        reveal_main(&handle);
    });
}

/// Managed marker holding the splash's shown-at `Instant`, so the `app_ready`
/// command can enforce the minimum-display window. Absent when the splash failed
/// to create (the main window was already revealed in that case).
pub struct SplashShownAt(pub Instant);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_dark_resolves_dark() {
        // Mirror authoritative: "dark" → dark regardless of OS.
        assert_eq!(
            resolve_theme(Some("dark"), SplashTheme::Light),
            SplashTheme::Dark
        );
    }

    #[test]
    fn mirror_light_resolves_light() {
        // Mirror authoritative: "light" → light regardless of OS.
        assert_eq!(
            resolve_theme(Some("light"), SplashTheme::Dark),
            SplashTheme::Light
        );
    }

    #[test]
    fn mirror_is_trimmed_before_matching() {
        // The renderer may write a trailing newline; it must still match.
        assert_eq!(
            resolve_theme(Some("dark\n"), SplashTheme::Light),
            SplashTheme::Dark
        );
        assert_eq!(
            resolve_theme(Some("  light  "), SplashTheme::Dark),
            SplashTheme::Light
        );
    }

    #[test]
    fn absent_mirror_falls_through_to_os() {
        // First launch (file absent) → OS theme, both directions.
        assert_eq!(resolve_theme(None, SplashTheme::Dark), SplashTheme::Dark);
        assert_eq!(resolve_theme(None, SplashTheme::Light), SplashTheme::Light);
    }

    #[test]
    fn garbage_mirror_falls_through_to_os() {
        // Any non-literal content (empty, junk, wrong case, partial) → OS theme.
        for junk in ["", "system", "DARK", "lite", "0", "{\"scheme\":\"dark\"}"] {
            assert_eq!(
                resolve_theme(Some(junk), SplashTheme::Light),
                SplashTheme::Light,
                "{junk:?} should fall through to the OS theme"
            );
            // And it never panics / always yields one of the two literals.
            let resolved = resolve_theme(Some(junk), SplashTheme::Dark);
            assert!(matches!(resolved, SplashTheme::Light | SplashTheme::Dark));
        }
    }

    #[test]
    fn theme_maps_to_query_value() {
        assert_eq!(SplashTheme::Light.as_query_value(), "light");
        assert_eq!(SplashTheme::Dark.as_query_value(), "dark");
    }

    #[test]
    fn tauri_theme_maps_with_light_default() {
        assert_eq!(SplashTheme::from(tauri::Theme::Dark), SplashTheme::Dark);
        assert_eq!(SplashTheme::from(tauri::Theme::Light), SplashTheme::Light);
    }

    #[test]
    fn reveal_guard_claims_exactly_once() {
        let guard = RevealGuard::default();
        // First claim wins; every subsequent claim (duplicate app_ready, the
        // safety timeout) is a no-op.
        assert!(guard.claim(), "first claim must win");
        assert!(!guard.claim(), "second claim must lose");
        assert!(!guard.claim(), "third claim must lose");
    }

    #[test]
    fn reveal_guard_is_thread_safe_single_winner() {
        use std::sync::Arc;
        use std::thread;

        // Many threads race to claim; EXACTLY one must win — this is the
        // invariant that prevents a double-reveal under concurrency (a duplicate
        // app_ready racing the safety timeout).
        let guard = Arc::new(RevealGuard::default());
        let winners: Arc<std::sync::atomic::AtomicUsize> =
            Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let g = Arc::clone(&guard);
                let w = Arc::clone(&winners);
                thread::spawn(move || {
                    if g.claim() {
                        w.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("thread panicked");
        }
        assert_eq!(
            winners.load(Ordering::SeqCst),
            1,
            "exactly one thread may win the reveal"
        );
    }

    // Smoke-test that the safety-timeout scheduling path uses the Tauri async
    // runtime (NOT a bare `tokio::spawn`, which panics with no reactor when
    // invoked from outside a Tokio context — the real `setup`-hook condition).
    //
    // This is deliberately a plain `#[test]` (no `#[tokio::test]`): a tokio test
    // would install an ambient reactor and MASK the very bug we guard against.
    // We assert that scheduling work the way `schedule_safety_reveal` /
    // `reveal_after_min_display` do — `tauri::async_runtime::spawn` of a future
    // that awaits a `tokio::time::sleep` — does not panic outside a Tokio context.
    #[test]
    fn async_runtime_spawn_works_without_ambient_tokio() {
        use std::sync::atomic::AtomicBool;

        let ran = Arc::new(AtomicBool::new(false));
        let ran_in = Arc::clone(&ran);
        // If this used `tokio::spawn`, it would panic here ("there is no reactor
        // running"). `tauri::async_runtime::spawn` owns its runtime, so it works.
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            ran_in.store(true, Ordering::SeqCst);
        });
        // Block on the handle from this non-async context to confirm it actually
        // ran to completion on the Tauri runtime.
        tauri::async_runtime::block_on(handle).expect("spawned task panicked");
        assert!(ran.load(Ordering::SeqCst), "spawned task must have run");
    }

    // ── Theme-mirror writer (set_theme_mirror backing logic) ──────────────────
    //
    // These exercise the writer against an explicit temp dir (NOT the env-backed
    // `data_dir()`), so no global state is touched and the architecture R4 env
    // rule is never tripped. The writer + reader share `theme_mirror_path_in`, so
    // a round-trip is a real reader↔writer agreement check.

    #[test]
    fn write_then_read_round_trips_each_scheme() {
        for scheme in ["light", "dark"] {
            let dir = tempfile::tempdir().expect("create temp dir");
            write_theme_mirror_in(dir.path(), scheme).expect("valid scheme must write");

            // File holds EXACTLY the literal (no newline, no quoting) …
            let raw = read_theme_mirror_from(dir.path()).expect("file must exist after write");
            assert_eq!(raw, scheme, "stored content must be the bare literal");

            // … and the real resolver round-trips it back to the same theme.
            let expected = if scheme == "dark" {
                SplashTheme::Dark
            } else {
                SplashTheme::Light
            };
            // OS fallback deliberately the OPPOSITE, so a pass proves the mirror
            // (not the fallback) decided the result.
            let os_opposite = if scheme == "dark" {
                SplashTheme::Light
            } else {
                SplashTheme::Dark
            };
            assert_eq!(
                resolve_theme(Some(&raw), os_opposite),
                expected,
                "{scheme:?} must resolve back through the reader"
            );
        }
    }

    #[test]
    fn write_trims_surrounding_whitespace_before_storing() {
        // The renderer might send a trailing newline; the stored value is the
        // canonical trimmed literal so the file never drifts from the closed set.
        let dir = tempfile::tempdir().expect("create temp dir");
        write_theme_mirror_in(dir.path(), "  dark\n").expect("trimmed-valid must write");
        assert_eq!(
            read_theme_mirror_from(dir.path()).as_deref(),
            Some("dark"),
            "stored content must be the trimmed literal, not the raw input"
        );
    }

    #[test]
    fn invalid_scheme_is_rejected_and_file_untouched() {
        // `"darkish"` shares a prefix with a valid value but is NOT exact; the
        // others cover empty, wrong-case, JSON, and a near-miss.
        for bad in [
            "blue",
            "",
            "darkish",
            "DARK",
            "light dark",
            "{\"scheme\":\"dark\"}",
        ] {
            let dir = tempfile::tempdir().expect("create temp dir");
            let err = write_theme_mirror_in(dir.path(), bad)
                .expect_err(&format!("{bad:?} must be rejected"));
            assert!(
                matches!(err, AppError::Validation(_)),
                "{bad:?} must be a Validation error, got {err:?}"
            );
            // Hard part of the contract: a rejected write creates NOTHING.
            assert!(
                read_theme_mirror_from(dir.path()).is_none(),
                "{bad:?} must not have created the mirror file"
            );
        }
    }

    #[test]
    fn invalid_scheme_does_not_clobber_an_existing_file() {
        // A previously-valid mirror must survive a later invalid write attempt —
        // the validation gate runs BEFORE any filesystem touch.
        let dir = tempfile::tempdir().expect("create temp dir");
        write_theme_mirror_in(dir.path(), "light").expect("seed valid value");
        let _ = write_theme_mirror_in(dir.path(), "purple");
        assert_eq!(
            read_theme_mirror_from(dir.path()).as_deref(),
            Some("light"),
            "an invalid write must leave the prior valid content intact"
        );
    }

    #[test]
    fn reader_and_writer_target_the_identical_path() {
        // The shared-path guarantee, asserted directly: both helpers derive the
        // file from `theme_mirror_path_in`, so they can never drift.
        let dir = tempfile::tempdir().expect("create temp dir");
        let expected = dir.path().join(THEME_MIRROR_FILE);
        assert_eq!(theme_mirror_path_in(dir.path()), expected);
    }
}
