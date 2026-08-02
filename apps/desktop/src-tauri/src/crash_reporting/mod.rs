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

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::commands::support::redact_lines;

/// Consent + "has the user been asked" state, persisted next to the app data.
const FILE_NAME: &str = "crash-reporting.json";

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

/// Read the persisted settings. Any failure — missing file, unreadable file,
/// corrupt JSON — yields the default, which does NOT transmit (because
/// `consent_shown` is false). Failing closed matters more than failing loud.
pub fn load(data_dir: &Path) -> Settings {
    std::fs::read_to_string(data_dir.join(FILE_NAME))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Persist settings. Best-effort: a write failure must never fail the caller's
/// operation, but it is logged because a silently unpersisted opt-OUT would
/// re-enable reporting on next launch.
pub fn save(data_dir: &Path, settings: Settings) {
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

/// Remove the persisted flag (factory reset). Back to default: enabled, not yet
/// consented — so the wizard asks again before anything is sent.
pub fn clear(data_dir: &Path) {
    let _ = std::fs::remove_file(data_dir.join(FILE_NAME));
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
pub fn init(data_dir: &Path) -> Option<sentry::ClientInitGuard> {
    let settings = load(data_dir);
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
        assert!(!load(&dir).transmits(), "missing file must fail closed");

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "{ not json").unwrap();
        assert!(!load(&dir).transmits(), "corrupt file must fail closed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_round_trips_and_clear_restores_the_default() {
        let dir = std::env::temp_dir().join("ajh-crash-reporting-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        save(
            &dir,
            Settings {
                enabled: false,
                consent_shown: true,
            },
        );
        let loaded = load(&dir);
        assert!(!loaded.enabled);
        assert!(loaded.consent_shown);

        clear(&dir);
        assert_eq!(load(&dir), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
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
