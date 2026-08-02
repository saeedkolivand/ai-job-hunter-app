//! Crash reporting — consent state + the redacted Sentry pipeline.
//!
//! Two responsibilities, deliberately in one small module because they are
//! useless apart: the **consent flag** that decides whether the SDK is created
//! at all, and the **redaction** every outgoing event passes through.
//!
//! ## Why the flag is a file and not a renderer preference
//!
//! Every other user preference lives in the renderer's `localStorage`
//! (`PreferencesSchema`). This one cannot: `sentry::init` has to run before
//! `tauri::Builder`, because `sentry-rust-minidump` forks the crash-reporter
//! process at startup and nothing after that fork can retroactively capture an
//! early native crash. There is no WebView at that point, so no `localStorage`.
//! The flag is therefore Rust-owned, in a small JSON file next to the other
//! app data, and the renderer reads/writes it over IPC.
//!
//! ## Transmission gate
//!
//! [`Settings::transmits`] is `enabled && consent_shown` — NOT just `enabled`.
//! The default is enabled, but nothing is sent until the setup wizard has
//! actually put that choice in front of the user. A default the user never saw
//! is not a choice, and the gap between a consent UI and what the code actually
//! does is where privacy claims break.
//!
//! ## Redaction
//!
//! Crash payloads are the richest source of accidental PII in the app: panic
//! messages interpolate paths, and every backtrace frame carries an absolute
//! source path containing the OS username. Everything leaving this module goes
//! through [`redact_event`], which reuses the same token redactor the
//! diagnostics bundle uses (ADR-027) rather than inventing a second, weaker one.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::commands::support::redact_lines;

/// Consent + "has the user been asked" state, persisted next to the app data.
const FILE_NAME: &str = "crash-reporting.json";

/// The directory holding the consent file, resolved exactly once.
///
/// This exists because the obvious implementation is silently broken. The two
/// sides of this feature run at very different moments:
///   * [`init`] runs at the top of `lib::run()`, BEFORE `tauri::Builder`, so it
///     has no `AppHandle`.
///   * the `privacy_*_crash_reporting` commands run long after `setup`.
///
/// `platform::config::data_dir()` is not stable across those two moments:
/// `setup` calls `resolve_and_export_data_dir`, which EXPORTS `AJH_DATA_DIR`
/// mid-process. So the same call returns `$HOME/.ajh` at startup and Tauri's
/// app-data dir afterwards — consent would be written to one directory and read
/// from another, and the feature would never activate no matter what the user
/// chose. It would also fail silently, because "no consent found" is
/// indistinguishable from "user said no".
///
/// Caching the first resolution makes both sides agree *by construction* rather
/// than by two call sites happening to resolve alike. Whichever directory wins,
/// it is the same one for reads, writes, and the factory-reset wipe.
static STATE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Resolve (once) and return the consent-file directory. First caller wins,
/// which is [`init`] at startup in the real app.
pub fn state_dir() -> &'static Path {
    STATE_DIR.get_or_init(crate::platform::config::data_dir)
}

/// Build-time ingest endpoint. `option_env!`, so a build without the secret —
/// every local `cargo build`, every contributor clone, every CI check that is
/// not the signed release job — compiles to `None` and can never transmit.
const DSN: Option<&str> = option_env!("AJH_SENTRY_DSN");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// User's choice. Defaults to on.
    pub enabled: bool,
    /// Whether the setup wizard has shown that choice yet.
    pub consent_shown: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            consent_shown: false,
        }
    }
}

impl Settings {
    /// The only predicate that may gate transmission. Enabled *and* asked.
    pub fn transmits(&self) -> bool {
        self.enabled && self.consent_shown
    }
}

/// Read the persisted settings from the resolved [`state_dir`].
pub fn load() -> Settings {
    load_from(state_dir())
}

/// Persist settings to the resolved [`state_dir`].
pub fn save(settings: Settings) {
    save_to(state_dir(), settings);
}

/// Remove the persisted flag from the resolved [`state_dir`] (factory reset).
/// Back to default: enabled, not yet consented — so the wizard asks again
/// before anything is sent.
pub fn clear() {
    let _ = std::fs::remove_file(state_dir().join(FILE_NAME));
}

/// Read the persisted settings. Any failure — missing file, unreadable file,
/// corrupt JSON — yields the default, which does NOT transmit (because
/// `consent_shown` is false). Failing closed matters more than failing loud.
///
/// Directory-taking so tests can drive it without touching the process-wide
/// [`STATE_DIR`]; production callers go through [`load`].
fn load_from(data_dir: &Path) -> Settings {
    std::fs::read_to_string(data_dir.join(FILE_NAME))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Persist settings. Best-effort: a write failure must never fail the caller's
/// operation, but it is logged because a silently unpersisted opt-OUT would
/// re-enable reporting on next launch.
fn save_to(data_dir: &Path, settings: Settings) {
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(data_dir.join(FILE_NAME), json)
    };
    if let Err(e) = write() {
        log::warn!("[crash-reporting] could not persist consent state: {e}");
    }
}

/// Redact every string in a serialized Sentry event.
///
/// Whole-event JSON round-trip rather than field-by-field: an event carries
/// paths in places that are easy to forget (frame `filename` and `abs_path`,
/// breadcrumb messages, `extra`, culprit, exception values), and a field list
/// is a denylist that silently rots as the SDK adds fields. Redacting the
/// serialized form is a allowlist-free way to cover all of them at once.
///
/// Only string *values* are touched; keys keep their structure so the event
/// still deserializes. Symbolication is unaffected — Sentry symbolicates from
/// `debug_meta` images and instruction addresses, not from source filenames.
fn redact_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            let redacted = redact_lines(s);
            if &redacted != s {
                *s = redacted;
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(redact_json),
        serde_json::Value::Object(map) => map.values_mut().for_each(redact_json),
        _ => {}
    }
}

/// Apply [`redact_json`] to a whole event. A serialization failure drops the
/// event entirely — an unredactable event is never worth sending.
fn redact_event(
    event: sentry::protocol::Event<'static>,
) -> Option<sentry::protocol::Event<'static>> {
    let mut json = serde_json::to_value(&event).ok()?;
    redact_json(&mut json);
    serde_json::from_value(json).ok()
}

/// Initialise Sentry when a DSN is baked in and the user's state permits it.
///
/// Returns the guard the caller must hold for the process lifetime. `None`
/// means the SDK was never created — a hard off, not a sampled-to-zero off, so
/// there is no client that could transmit even if something later tried.
pub fn init() -> Option<sentry::ClientInitGuard> {
    // First call to `state_dir()` in the real app — this is what pins the
    // directory that the privacy commands will later read and write.
    let settings = load();
    if !settings.transmits() {
        return None;
    }
    let dsn = DSN?;

    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(env!("CARGO_PKG_VERSION").into()),
            environment: Some(if cfg!(debug_assertions) {
                "development".into()
            } else {
                "production".into()
            }),
            // Release health: crash-free rate and version adoption. This is the
            // "usage" half of the feature — active installs per version — and it
            // needs no separate analytics vendor.
            auto_session_tracking: true,
            session_mode: sentry::SessionMode::Application,
            // Never attach the request/user identity the SDK can infer.
            send_default_pii: false,
            // The SDK defaults this to the machine hostname, which on a personal
            // device is frequently the user's real name.
            server_name: Some("redacted".into()),
            before_send: Some(Arc::new(redact_event)),
            before_breadcrumb: Some(Arc::new(|mut breadcrumb| {
                breadcrumb.message = breadcrumb.message.map(|m| redact_lines(&m));
                Some(breadcrumb)
            })),
            ..Default::default()
        },
    )))
}

/// Stop capturing in the current process, immediately.
///
/// Unbinding the client from the hub drops every subsequent event on the floor
/// without waiting for a restart — someone who has just switched reporting off
/// should not keep being reported for the rest of the session.
///
/// This cannot recall the minidump supervisor: it is a separate process forked
/// before the WebView existed, so a hard native crash before the next restart
/// may still be delivered. That limitation is stated in the settings copy rather
/// than papered over.
pub fn disable_current() {
    sentry::Hub::current().bind_client(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_does_not_transmit_until_consent_is_shown() {
        let d = Settings::default();
        assert!(d.enabled, "default is opt-out, not opt-in");
        assert!(!d.consent_shown);
        assert!(
            !d.transmits(),
            "a default the user has not been shown must not transmit"
        );
    }

    #[test]
    fn transmits_only_when_enabled_and_shown() {
        let shown_on = Settings {
            enabled: true,
            consent_shown: true,
        };
        let shown_off = Settings {
            enabled: false,
            consent_shown: true,
        };
        assert!(shown_on.transmits());
        assert!(!shown_off.transmits());
    }

    #[test]
    fn load_falls_back_to_the_non_transmitting_default() {
        let dir = std::env::temp_dir().join("ajh-crash-reporting-missing");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !load_from(&dir).transmits(),
            "missing file must fail closed"
        );

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "{ not json").unwrap();
        assert!(
            !load_from(&dir).transmits(),
            "corrupt file must fail closed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_round_trips_and_clear_restores_the_default() {
        let dir = std::env::temp_dir().join("ajh-crash-reporting-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        save_to(
            &dir,
            Settings {
                enabled: false,
                consent_shown: true,
            },
        );
        let loaded = load_from(&dir);
        assert!(!loaded.enabled);
        assert!(loaded.consent_shown);

        let _ = std::fs::remove_file(dir.join(FILE_NAME));
        assert_eq!(load_from(&dir), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression guard for the bug this module's `STATE_DIR` exists to prevent.
    ///
    /// The original implementation resolved the directory separately on each
    /// side: `init` called `platform::config::data_dir()` before `setup`, while
    /// the privacy commands used `app.path().app_data_dir()`. Because `setup`
    /// exports `AJH_DATA_DIR` mid-process, those resolved to DIFFERENT
    /// directories — consent was written to one and read from the other, so the
    /// feature never activated and did so silently, since "no file" reads as
    /// "not consented".
    ///
    /// Pinning that every accessor funnels through one cached resolution is what
    /// makes read/write agreement structural instead of coincidental.
    #[test]
    fn every_accessor_shares_one_resolved_directory() {
        // Mutating AJH_DATA_DIR would race other tests in this binary, so assert
        // on identity: repeated resolution is stable even though the underlying
        // env-var-dependent resolver is not.
        let first = state_dir();
        let second = state_dir();
        assert!(
            std::ptr::eq(first, second),
            "state_dir must hand back one cached path, not re-resolve per call"
        );

        // And the round-trip helpers must agree with it: writing through the
        // public API must be readable through the public API.
        let before = load();
        save(Settings {
            enabled: false,
            consent_shown: true,
        });
        let after = load();
        assert!(
            !after.enabled && after.consent_shown,
            "save() must be observable through load() — they resolved to different dirs otherwise"
        );
        assert!(!after.transmits(), "an explicit opt-out must not transmit");

        // Restore whatever the environment had, so this test leaves no trace.
        if before == Settings::default() {
            clear();
        } else {
            save(before);
        }
    }

    /// The gate that protects the privacy claim: nothing identifying may survive
    /// into a transmitted event.
    #[test]
    fn redact_json_scrubs_paths_urls_credentials_and_emails() {
        let mut json = serde_json::json!({
            "message": "panic at C:\\Users\\alice\\project\\src\\main.rs while calling https://api.example.com/v1",
            "nested": {
                "frames": [
                    { "filename": "/home/alice/dev/app/src/lib.rs" },
                    { "note": "contact alice@example.com token=sk-secret-value" }
                ]
            },
            "count": 7,
            "flag": true
        });
        redact_json(&mut json);
        let dumped = serde_json::to_string(&json).unwrap();

        for leaked in [
            "alice",
            "api.example.com",
            "sk-secret-value",
            "alice@example.com",
        ] {
            assert!(
                !dumped.contains(leaked),
                "`{leaked}` survived redaction in: {dumped}"
            );
        }
        // Non-string values must survive untouched — redaction must not corrupt
        // the event shape.
        assert_eq!(json["count"], 7);
        assert_eq!(json["flag"], true);
        // Human-readable structure is preserved around the redactions.
        assert!(json["message"].as_str().unwrap().contains("panic at"));
    }
}
