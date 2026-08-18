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
//! The flag is therefore Rust-owned in a small JSON file, and the renderer
//! reads and writes it over IPC.
//!
//! ## Where the file lives, and why it is NOT the OS app-data dir
//!
//! It lives in the directory [`crate::platform::config::data_dir`] resolves to
//! *before* Tauri starts: `$AJH_DATA_DIR` if the user set one, else
//! `$HOME/.ajh`. In a default install that is **not** the per-OS app-data
//! directory the rest of the app's stores use.
//!
//! That is forced, not sloppy. Tauri's authoritative `app_data_dir()` needs an
//! `AppHandle`, which does not exist this early, and `setup` — which resolves it
//! and exports `AJH_DATA_DIR` — runs strictly later. Moving `init` into `setup`
//! to get the handle is not an option either: the minidump supervisor re-executes
//! everything above its own call in the forked child, so a late fork would have
//! the child build a second Tauri app.
//!
//! Consequence worth knowing: deleting the app-data directory by hand does not
//! remove this file. It holds two booleans and no personal data, and the factory
//! reset in `commands::privacy` does clear it, so nothing user-identifying
//! survives — but the file itself is elsewhere.
//!
//! ## Transmission gate
//!
//! [`Settings::transmits`] is `enabled && consent_shown` — NOT just `enabled`.
//! The default is enabled, but nothing is sent until the setup wizard has
//! actually put that choice in front of the user. A default the user never saw
//! is not a choice, and the gap between a consent UI and what the code actually
//! does is where privacy claims break.
//!
//! ## Redaction, and where it is actually enforced
//!
//! Crash payloads are the richest source of accidental PII in the app: panic
//! messages interpolate paths, and every backtrace frame carries an absolute
//! source path containing the OS username.
//!
//! Two mechanisms, at two different depths, because one of them is not enough:
//!
//! * [`redact_event`] runs as `before_send` and rewrites events on the
//!   **capture path**, reusing the same token redactor the diagnostics bundle
//!   uses (ADR-027) rather than inventing a second, weaker one. It is an
//!   event-shaping hook — it only sees what `capture_event` prepared.
//! * [`transport`] is the **wire gate**, and it is what the privacy claim
//!   actually rests on. `before_send` is not the last hop: an envelope handed
//!   straight to `Client::send_envelope` never reaches it, and
//!   `tauri-plugin-sentry` 0.6 does exactly that for renderer envelopes it
//!   cannot parse. The transport re-checks consent and drops anything opaque,
//!   for every path, on every envelope. See that module for the full chain.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::commands::support::redact_lines;
use crate::observability::sanitize_reason;

mod transport;

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
/// which is [`init`] at startup in the real app — so in practice this pins the
/// PRE-setup resolution (`$AJH_DATA_DIR` or `$HOME/.ajh`), deliberately, not the
/// OS app-data dir. See the module docs for why that is forced.
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

/// The consent answer as the **wire gate** sees it, mirrored out of the file so
/// the transport can re-check it per envelope without touching the disk.
///
/// Starts `false` so it fails closed: a transport that somehow ran before
/// [`init`] transmits nothing. Every place that changes the persisted answer
/// ([`init`], [`save`], [`clear`], [`disable_current`]) updates this too — that
/// is the whole contract, and it is small enough to keep honest by inspection.
static TRANSMITS: AtomicBool = AtomicBool::new(false);

/// Read the wire gate. A single relaxed atomic load: no lock, no allocation, no
/// syscall, so it is safe to call on the transport's sender thread per envelope.
pub(crate) fn transmits_now() -> bool {
    TRANSMITS.load(Ordering::Relaxed)
}

/// Read the persisted settings from the resolved [`state_dir`].
pub fn load() -> Settings {
    load_from(state_dir())
}

/// Persist settings to the resolved [`state_dir`] and move the wire gate with
/// them, so an opt-out takes effect on the next envelope rather than the next
/// launch.
pub fn save(settings: Settings) {
    save_to(state_dir(), settings);
    TRANSMITS.store(settings.transmits(), Ordering::Relaxed);
}

/// Remove the persisted flag from the resolved [`state_dir`] (factory reset).
/// Back to default: enabled, not yet consented — so the wizard asks again
/// before anything is sent, and the wire gate closes immediately.
pub fn clear() {
    let _ = std::fs::remove_file(state_dir().join(FILE_NAME));
    TRANSMITS.store(Settings::default().transmits(), Ordering::Relaxed);
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
        log::warn!(
            "[crash-reporting] could not persist consent state: {}",
            sanitize_reason(&e.to_string())
        );
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
    // Open the wire gate here and nowhere else at startup: the transport is
    // built inside `sentry::init` below, and it must never observe the
    // fail-closed default while the client it belongs to is live.
    TRANSMITS.store(settings.transmits(), Ordering::Relaxed);
    if !settings.transmits() {
        return None;
    }
    let dsn = DSN?;

    Some(sentry::init((dsn, client_options())))
}

/// The client configuration, split out from [`init`] so the privacy-relevant
/// answers in it are assertable without a DSN or a live client.
///
/// Builder rather than a struct literal: `ClientOptions` is `#[non_exhaustive]`
/// as of sentry 0.49, so `..Default::default()` no longer compiles from a
/// downstream crate. Every setter below is still one field, set explicitly —
/// including the ones the SDK now happens to default the same way, because a
/// privacy-relevant value should be readable here, not inferred from an
/// upstream default that can change under us.
fn client_options() -> sentry::ClientOptions {
    sentry::ClientOptions::new()
        .release(env!("CARGO_PKG_VERSION"))
        .environment(if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        })
        // Release health: crash-free rate and version adoption. This is the
        // "usage" half of the feature — active installs per version — and it
        // needs no separate analytics vendor.
        .auto_session_tracking(true)
        .session_mode(sentry::SessionMode::Application)
        // Never attach the request/user identity the SDK can infer.
        .send_default_pii(false)
        // The SDK defaults this to the machine hostname, which on a personal
        // device is frequently the user's real name.
        .server_name("redacted")
        // Structured logs and metrics: two egress pipelines 0.49 turned ON
        // by default (0.42 defaulted both off). The `logs`/`metrics` cargo
        // features are off, so today the consumers are cfg'd out — but any
        // future crate that enables `sentry/logs` would unify the feature
        // into our build and silently open the pipe. These setters compile
        // with the features off, so pinning the answer here costs nothing
        // and stops that from ever being a silent change.
        .enable_logs(false)
        .enable_metrics(false)
        .before_send(redact_event)
        .before_breadcrumb(|mut breadcrumb| {
            breadcrumb.message = breadcrumb.message.map(|m| redact_lines(&m));
            Some(breadcrumb)
        })
        // The wire gate. Everything above shapes events; this decides what
        // is allowed to leave the process at all. See `transport`.
        .transport(transport::GuardedTransportFactory)
}

/// Stop transmitting in the current process, immediately.
///
/// Closing [`TRANSMITS`] is what actually stops it: the wire gate in
/// [`transport`] re-reads that flag for every envelope, so the next one is
/// dropped no matter which path produced it.
///
/// The hub unbind is the second, weaker half and is kept only because it also
/// stops events being *built*. On its own it would not be enough, and the
/// earlier version of this doc — which claimed unbinding "drops every
/// subsequent event on the floor" — was simply false on two counts:
///   * `Hub::current()` is **thread-local**, so this unbinds one thread. Work
///     on any other thread keeps its own hub, and its own client.
///   * `tauri-plugin-sentry` hands its own `Client` clone to Tauri state and
///     calls `send_envelope` on it directly, never consulting a hub at all.
///
/// Neither can recall the minidump supervisor: it is a separate process forked
/// before the WebView existed, holding its own client and its own copy of the
/// gate's startup value, so a hard native crash before the next restart may
/// still be delivered. That limitation is stated in the settings copy rather
/// than papered over.
pub fn disable_current() {
    TRANSMITS.store(false, Ordering::Relaxed);
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
        // The wire gate must move with the file. If `save` ever stops mirroring
        // into `TRANSMITS`, an opt-out would persist to disk while the transport
        // kept sending for the rest of the session.
        assert!(
            !transmits_now(),
            "save(opt-out) must close the wire gate, not just write the file"
        );

        // Same for the consent-granted direction, so the guard cannot pass by
        // being stuck closed.
        save(Settings {
            enabled: true,
            consent_shown: true,
        });
        assert!(
            transmits_now(),
            "save(consent granted) must open the wire gate"
        );

        // ...and the factory-reset path must close it again.
        clear();
        assert!(
            !transmits_now(),
            "clear() restores the never-asked default, which must not transmit"
        );

        // Restore whatever the environment had, so this test leaves no trace.
        if before == Settings::default() {
            clear();
        } else {
            save(before);
        }
    }

    /// The privacy-relevant half of the client configuration, pinned.
    ///
    /// These are switches whose wrong value leaks something and whose *absence*
    /// is invisible at runtime: `enable_logs`/`enable_metrics` default to TRUE
    /// in sentry 0.49, `server_name` defaults to the machine hostname, and
    /// deleting the `.transport(...)` call would remove the entire wire gate
    /// while everything still compiled and shipped. Each of those was checked by
    /// deleting the builder call and watching this test go red.
    ///
    /// One exception, stated rather than glossed: `send_default_pii` is `false`
    /// in `ClientOptions::default()` too, so that assertion catches someone
    /// setting it TRUE but not someone deleting our explicit `false`. There is
    /// nothing observable that would distinguish the two, so it is a value pin,
    /// not a guard — and this comment is the honest version of that.
    #[test]
    fn client_options_pin_every_privacy_switch() {
        let options = client_options();

        assert!(
            options.transport.is_some(),
            "the wire gate must be installed — without it raw envelopes egress unredacted"
        );
        assert!(
            options.before_send.is_some(),
            "capture-path events must still be redacted"
        );
        assert!(
            options.before_breadcrumb.is_some(),
            "breadcrumb messages must still be redacted"
        );
        assert!(
            !options.enable_logs,
            "0.49 defaults structured logs ON; that is egress we never consented to"
        );
        assert!(
            !options.enable_metrics,
            "0.49 defaults metrics ON; that is egress we never consented to"
        );
        assert!(
            !options.send_default_pii,
            "the SDK must never attach inferred user identity"
        );
        assert_eq!(
            options.server_name.as_deref(),
            Some("redacted"),
            "server_name defaults to the hostname, which is often the user's real name"
        );
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
