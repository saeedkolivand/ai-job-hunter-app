use super::*;

/// Cargo features on `sentry` that would open a log or metric pipeline: the
/// two telemetry features themselves, plus the two integrations whose whole
/// job is capturing `log`/`tracing` records automatically.
const FORBIDDEN_SENTRY_FEATURES: &[&str] = &["logs", "metrics", "log", "tracing"];

/// Read the `sentry = { … }` inline table out of a Cargo manifest.
///
/// Returns `(inherits_defaults, quoted_values)` — the second being every
/// quoted string in the table, i.e. the version followed by the explicitly
/// enabled features. Whole strings, never substrings, so `release-health`
/// cannot be misread as `log`.
///
/// Text, because `#[cfg(feature = …)]` sees *our* crate's features and never
/// a dependency's; same shape `tests/egress.rs` uses for its declaration
/// files. Takes the manifest as an argument purely so the test below can
/// feed it mutated inputs.
fn sentry_declaration(manifest: &str) -> (bool, Vec<&str>) {
    let decl = manifest
        .split_once("\nsentry = {")
        .expect("`sentry` must still be declared as an inline table in Cargo.toml")
        .1
        .split_once("] }")
        .expect("the `sentry` declaration must still end with `features = [ … ] }`")
        .0;
    let inherits_defaults = !decl.replace(' ', "").contains("default-features=false");
    let values = decl.split('"').skip(1).step_by(2).collect();
    (inherits_defaults, values)
}

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
/// is invisible at runtime: `server_name` defaults to the machine hostname,
/// and deleting the `.transport(...)` call would remove the entire wire gate
/// while everything still compiled and shipped. Each of those was checked by
/// deleting the builder call and watching this test go red.
///
/// The logs/metrics half of this test moved out rather than being dropped:
/// sentry 0.49.2 deprecated `enable_logs`/`enable_metrics`, and those
/// switches were never the gate they read as (see the module doc). The
/// integration assertion below is what remains of them here — the rest is
/// `sentry_log_and_metric_pipelines_are_off_at_the_feature_gate`.
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
        options.integrations.is_empty(),
        "we register no custom integration — the log- and tracing-capture ones are the only \
             way an event source other than a panic gets attached, and they are never automatic"
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

/// Structured logs and metrics are two egress pipelines ADR-0020 never
/// consented to, and after sentry 0.49.2 there is no `ClientOptions` switch
/// left that turns either off — `enable_metrics` is a no-op and
/// `enable_logs` only muted automatic capture. The claim therefore rests on
/// the **dependency declaration**, so that is what this pins: sentry taken
/// with `default-features = false` (its default set contains both `logs`
/// and `metrics`) and an explicit feature list that names neither those nor
/// the two capture integrations, `log` and `tracing`.
///
/// Limit, stated rather than glossed: this pins the declaration we own. A
/// third-party crate that enabled `sentry/logs` would unify the feature in
/// and leave this test green. That alone still sends nothing — a log needs
/// a call site or an installed integration — which is why
/// `egress_no_source_captures_a_sentry_log_or_metric` (`tests/egress.rs`)
/// and the `integrations.is_empty()` assertion above are the other two
/// legs, and none of the three is redundant.
#[test]
fn sentry_log_and_metric_pipelines_are_off_at_the_feature_gate() {
    let (inherits_defaults, enabled) = sentry_declaration(include_str!("../../Cargo.toml"));

    assert!(
        !inherits_defaults,
        "sentry must keep `default-features = false` — its default feature set includes \
             `logs` and `metrics`, the two egress pipelines ADR-0020 never consented to"
    );
    assert!(
        enabled.contains(&"backtrace"),
        "extractor broke: the sentry feature list should contain `backtrace`, got {enabled:?}"
    );
    for forbidden in FORBIDDEN_SENTRY_FEATURES {
        assert!(
            !enabled.contains(forbidden),
            "sentry feature `{forbidden}` would open a structured-log or metric pipeline the \
                 crash-reporting consent (ADR-0020) does not cover; it also makes the SDK's \
                 log-capture and metric-builder APIs compile, so the module doc's \"this crate \
                 cannot capture a log\" claim would stop being true. Enabled: {enabled:?}"
        );
    }
}

/// [`sentry_declaration`] is only a guard if it can go red, and the real
/// manifest cannot be mutated from a test — so mutate the *input* instead.
/// Both ways a pipeline reopens are exercised, plus a clean control so the
/// reader is not simply always-alarming.
#[test]
fn sentry_declaration_reader_catches_both_ways_a_pipeline_reopens() {
    // 1. Defaults inherited: sentry's default set carries `logs` and
    //    `metrics` even though the explicit list names neither, so the
    //    forbidden-name scan alone would miss this entirely.
    let inherited = "\nsentry = { version = \"0.49\", features = [\n  \"backtrace\",\n] }\n";
    let (inherits_defaults, enabled) = sentry_declaration(inherited);
    assert!(
        inherits_defaults,
        "a missing `default-features = false` must be flagged"
    );
    assert!(!enabled
        .iter()
        .any(|f| FORBIDDEN_SENTRY_FEATURES.contains(f)));

    // 2. A pipeline feature named outright while defaults are correctly
    //    disabled — the case the other assertion must catch on its own.
    let named = "\nsentry = { version = \"0.49\", default-features = false, features = [\n  \"backtrace\",\n  \"logs\",\n] }\n";
    let (inherits_defaults, enabled) = sentry_declaration(named);
    assert!(!inherits_defaults);
    assert!(
        enabled.contains(&"logs"),
        "an explicitly enabled `logs` must be flagged"
    );

    // 3. Control: the shape the real manifest has must come back clean, or
    //    the two cases above could pass for the wrong reason. `release-health`
    //    also proves whole-string matching — a substring scan would read
    //    `log` out of it.
    let clean = "\nsentry = { version = \"0.49\", default-features = false, features = [\n  \"backtrace\",\n  \"release-health\",\n] }\n";
    let (inherits_defaults, enabled) = sentry_declaration(clean);
    assert!(!inherits_defaults);
    assert!(!enabled
        .iter()
        .any(|f| FORBIDDEN_SENTRY_FEATURES.contains(f)));
    assert!(enabled.contains(&"release-health"));
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
