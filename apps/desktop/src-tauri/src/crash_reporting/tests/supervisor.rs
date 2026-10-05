//! The native-crash supervisor wiring (`sentry-minidump`'s `MinidumpIntegration`, registered by
//! `client_options`): the reporter process must start with no argv, and what it reports about
//! its own failures must be redacted like any other event.

use super::super::*;

/// The crash-reporter process must start with NO argv, exactly as the
/// `sentry-rust-minidump` 0.17 child always did: `MinidumpIntegration` defaults
/// to forwarding the app's argv, and ours can carry an `ajh://` deep link.
///
/// `inherit_args` has no getter, so this reads the integration's `Debug`
/// output (which prints it) after downcasting it out of the options, the same
/// `as_ref().as_any()` dispatch `sentry-minidump` uses on its own integration.
/// It compares against `new().inherit_args(false)` rather than grepping one
/// field, which also pins every other visible knob (crashes dir, flush
/// timeout, process name, …) to its default; the stock `new()` is checked to
/// differ, so the comparison can actually go red.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn minidump_supervisor_does_not_inherit_argv_and_keeps_every_other_default() {
    use sentry::integrations::minidump::MinidumpIntegration;

    let options = client_options();
    let ours = options
        .integrations
        .iter()
        .find_map(|i| i.as_ref().as_any().downcast_ref::<MinidumpIntegration>())
        .expect("client_options must register the MinidumpIntegration");

    let ours = format!("{ours:?}");
    assert!(
        ours.contains("inherit_args: false"),
        "the reporter must not receive the app's argv (it can carry an `ajh://` deep link): {ours}"
    );
    assert_eq!(
        ours,
        format!("{:?}", MinidumpIntegration::new().inherit_args(false)),
        "only `inherit_args` may differ from the integration's defaults"
    );
    assert_ne!(
        ours,
        format!("{:?}", MinidumpIntegration::new()),
        "control: the stock integration must read differently, or the pin above proves nothing"
    );
}

/// `argv` can carry an `ajh://` deep link, so it must never reach the reporter.
///
/// The `Debug` pin above cannot see an `on_process` closure (the one hook that
/// could `.args(...)` the app's argv back in) or `before_capture`, because
/// `MinidumpIntegration`'s `Debug` omits both. So this scans the source that
/// builds the integration instead — the repo's usual text-scan shape (see
/// `sentry_declaration` in the parent `tests.rs`). Comment lines are skipped so prose can still say why.
#[test]
fn crash_supervisor_wiring_never_re_forwards_argv() {
    let code: String = include_str!("../mod.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [".on_process(", "inherit_args(true)"] {
        assert!(
            !code.contains(forbidden),
            "`{forbidden}` in crash_reporting/mod.rs can hand the app's argv (which may carry an \
             `ajh://` deep link) to the crash-reporter process; the Debug-output pin cannot see \
             an `on_process` closure, so this text scan is the only guard"
        );
    }
}

/// When the crash-reporter's server fails to start, `sentry-minidump` reports
/// the error itself — and its text embeds the socket name `Debug`-printed as
/// `Path("<temp dir>/…")`. Run through the REAL entry point (`before_send`'s
/// [`redact_event`]), neither the macOS `/var/folders/…` nor the Windows
/// `D:\Temp\<user>\…` temp dir may survive, while the sentence stays readable.
#[test]
fn a_crash_reporter_startup_error_does_not_leak_the_temp_dir() {
    use sentry::protocol::{Event, Exception};

    for (socket, leaked) in [
        (
            r#"Path("/var/folders/ab/xyz123/T/temp-socket-0f")"#,
            "xyz123",
        ),
        (r#"Path("D:\\Temp\\alice\\temp-socket-0f")"#, "alice"),
    ] {
        let message = format!("Failed to create server with socket name {socket}");
        let event = Event {
            message: Some(message.clone()),
            exception: vec![Exception {
                ty: "Error".into(),
                value: Some(message),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        let redacted = redact_event(event).expect("a plain event must survive redaction");
        let value = redacted.exception.values[0].value.clone().unwrap();
        assert_eq!(
            value, r#"Failed to create server with socket name Path("<path-redacted>")"#,
            "exception value for {socket}"
        );
        let dumped = serde_json::to_string(&redacted).unwrap();
        assert!(!dumped.contains(leaked), "`{leaked}` survived in: {dumped}");
    }
}
