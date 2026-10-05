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
//! `tauri::Builder`, because `sentry`'s `MinidumpIntegration` (`sentry-minidump`)
//! forks the crash-reporter process inside it, at startup, and nothing after
//! that fork can retroactively capture an early native crash. There is no
//! WebView at that point, so no `localStorage`.
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
//! everything above `sentry::init` in the forked child, so a late fork would
//! have the child build a second Tauri app.
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
//!   `tauri-plugin-sentry` (0.7) does exactly that for renderer envelopes it
//!   cannot parse. The transport re-checks consent and drops anything opaque,
//!   for every path, on every envelope. See that module for the full chain.
//!
//! ## Structured logs and metrics: off at the feature gate, not at a switch
//!
//! sentry 0.49 added two egress pipelines next to crash events — structured
//! logs and metrics — and **0.49.2**, the version this build resolves to
//! (`Cargo.toml` asks for `"0.49"`; `Cargo.lock` pins 0.49.2), then deprecated
//! both `ClientOptions` switches that appeared to control them, which is why
//! [`client_options`] sets neither. The deprecation attributes are the whole
//! argument, so they are quoted verbatim from
//! `sentry-core-0.49.2/src/clientoptions.rs` — on the next bump, diff these
//! two sentences rather than re-deriving the reasoning:
//!
//! * `enable_logs`: *"logs captured manually are always sent; only automatic
//!   capture by integrations respects this option"*.
//! * `enable_metrics`: *"this option is a deprecated no-op"* (the field's own
//!   doc adds "Metrics are always enabled, regardless of this option's value";
//!   only a *call* to `sentry::metrics::*` emits one).
//!
//! Read literally, the first sentence says a manual `capture_log` would be
//! sent no matter what that switch said — so dropping it is safe **here** on
//! two facts about this build, not on the switch having been useless in
//! general: no `log`/`tracing` capture integration is installed (there is no
//! automatic capture left for it to have muted), and the `logs` feature is off
//! (the manual API the sentence promises to always send does not compile). If
//! either fact changes — a bump that reworded the attributes, or a dependency
//! that unifies `sentry/logs` back into the build — this section is what went
//! stale, and the three tests named below are what say so.
//!
//! Both facts live in the dependency declaration. `sentry` is
//! taken with `default-features = false` and an explicit list that omits
//! `logs`, `metrics`, `log` and `tracing`, and the consequence is stronger than
//! a runtime flag:
//!
//! * the whole log API (`Hub::capture_log`, the `logger_*` macros) is
//!   `#[cfg(feature = "logs")]`, so this crate *cannot* capture a log — it
//!   would not compile — and in fact never tries;
//! * no log-capturing integration is registered. `sentry::apply_defaults` adds
//!   only the backtrace, debug-images, contexts and panic integrations; the
//!   `log`/`tracing` ones are never automatic, and the one integration
//!   [`client_options`] adds itself is the `MinidumpIntegration` crash
//!   supervisor, which captures native crashes and no `log`/`tracing` record.
//!
//! Three tests, one per leg, none of them redundant:
//! `sentry_log_and_metric_pipelines_are_off_at_the_feature_gate` (the feature
//! list), the "only the minidump supervisor is registered" assertion in
//! `client_options_pin_every_privacy_switch` (nothing else registered), and
//! `egress_no_source_captures_a_sentry_log_or_metric` in `tests/egress.rs` (no
//! call site anywhere in `src/`). The last is the one that still bites if a
//! *third-party* crate ever unifies `sentry/logs` into the build: the feature
//! returning does not by itself send anything, but a call site would, and that
//! is what goes red.
//!
//! Note the `log::warn!`/`log::debug!` calls in this module and in
//! [`transport`] are the `log` **facade**, routed to `tauri-plugin-log` on
//! disk. They reach Sentry only via `sentry-log`, which is not in the build.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::commands::support::redact_lines;
use crate::observability::sanitize_reason;
use crate::platform::fs::write_atomic;

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
        write_atomic(&data_dir.join(FILE_NAME), json.as_bytes())
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
    let options = sentry::ClientOptions::new()
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
        // No `.enable_logs(false)` / `.enable_metrics(false)` here any more:
        // sentry 0.49.2 deprecated both, and reading why is what moved the
        // guarantee. `enable_metrics` is a documented no-op — metrics are
        // always enabled and only a *call* to `sentry::metrics::*` emits one.
        // `enable_logs(false)` only ever muted the automatic `log`/`tracing`
        // capture integrations; "logs captured manually are always sent". So
        // neither switch was ever the gate it looked like.
        //
        // The gate is the feature list in `Cargo.toml`. Without `sentry/logs`
        // the entire log API — `Hub::capture_log`, the `logger_*` macros — is
        // `#[cfg(feature = "logs")]`-compiled out, so a manual capture is a
        // compile error rather than a silent send; and `sentry::apply_defaults`
        // registers only the backtrace/debug-images/contexts/panic
        // integrations, never a log-capturing one (those must be installed by
        // hand, and the only integration installed here is the minidump
        // supervisor, via `with_crash_supervisor`). See the module doc, and the tests that pin
        // it: `sentry_log_and_metric_pipelines_are_off_at_the_feature_gate`
        // below plus `egress_no_source_captures_a_sentry_log_or_metric` in
        // `tests/egress.rs`.
        .before_send(redact_event)
        .before_breadcrumb(|mut breadcrumb| {
            breadcrumb.message = breadcrumb.message.map(|m| redact_lines(&m));
            Some(breadcrumb)
        })
        // The wire gate. Everything above shapes events; this decides what
        // is allowed to leave the process at all. See `transport`.
        .transport(transport::GuardedTransportFactory);
    with_crash_supervisor(options)
}

/// Register the native-crash supervisor — the ONLY integration we add.
///
/// Inside `sentry::init` it re-executes this binary as the crash-reporter
/// process, which rebuilds its client from these same options (minus this
/// integration), so `before_send` and the wire gate apply to the minidump event
/// (and `before_breadcrumb` to any breadcrumb synced over). `inherit_args(false)`
/// is load-bearing: the reporter must start with NO argv, as it always has — the
/// app's argv can carry an `ajh://` deep link, which has no business in a second
/// process. Every other knob stays at the integration's default.
///
/// Split out and gated on the targets `sentry` compiles the module for, so the
/// other targets get an identity function instead of a `let x = …; x` that
/// clippy would reject.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn with_crash_supervisor(options: sentry::ClientOptions) -> sentry::ClientOptions {
    options.add_integration(
        sentry::integrations::minidump::MinidumpIntegration::new().inherit_args(false),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn with_crash_supervisor(options: sentry::ClientOptions) -> sentry::ClientOptions {
    options
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
mod tests;
