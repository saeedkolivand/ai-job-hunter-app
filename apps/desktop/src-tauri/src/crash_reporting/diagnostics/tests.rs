use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sentry::protocol::EnvelopeItem;
use sentry::{Client, ClientOptions, Envelope, Hub, Scope, Transport};

use super::*;
use crate::crash_reporting::redact_event;
use crate::error::AppError;

/// Real `AppError::code()` values, so the fixtures cannot drift from the wire.
fn storage_code() -> &'static str {
    AppError::Storage("x".into()).code()
}

/// Records every envelope instead of sending it.
#[derive(Default)]
struct Capture(Mutex<Vec<Envelope>>);

impl Transport for Capture {
    fn send_envelope(&self, envelope: Envelope) {
        self.0.lock().unwrap().push(envelope);
    }
}

/// Run `f` on a hub whose client has the app's real `before_send(redact_event)` and a
/// capturing transport. The SDK only treats a client as active with a DSN and a
/// transport, hence the dummy DSN; nothing leaves the process.
fn with_capturing_hub<R>(f: impl FnOnce() -> R) -> (R, Vec<Envelope>) {
    let capture = Arc::new(Capture::default());
    let options = ClientOptions::new()
        .dsn("https://key@sentry.invalid/1")
        .before_send(redact_event)
        .transport(capture.clone());
    let hub = Arc::new(Hub::new(
        Some(Arc::new(Client::from(options))),
        Arc::new(Scope::default()),
    ));
    let out = Hub::run(hub, f);
    let sent = std::mem::take(&mut *capture.0.lock().unwrap());
    (out, sent)
}

fn current_tags() -> BTreeMap<String, String> {
    let mut tags = Default::default();
    Hub::current().configure_scope(|scope| {
        tags = scope.apply_to_event(Event::default()).unwrap().tags;
    });
    tags
}

#[test]
fn store_open_event_is_one_event_for_all_failures() {
    let code = storage_code();
    let event =
        store_open_failed_event(&[("ai_provider_config", code), ("documents", "CONFIG")]).unwrap();
    assert_eq!(event.level, Level::Error);
    assert_eq!(event.message.as_deref(), Some("store failed to open"));
    assert_eq!(
        event.fingerprint.iter().map(|s| &**s).collect::<Vec<_>>(),
        ["store-open-failed", "ai_provider_config", code]
    );
    assert_eq!(event.tags["store"], "ai_provider_config");
    assert_eq!(event.tags["error_code"], code);
    assert_eq!(event.tags["store_count"], "2");
    assert_eq!(event.tags["stores"], "ai_provider_config,documents");
    assert_eq!(event.tags.len(), 4);
    assert!(store_open_failed_event(&[]).is_none());
}

#[test]
fn stores_tag_is_capped() {
    let labels = ["a_long_store_label_for_the_cap_test"; 20];
    let failures: Vec<(&str, &str)> = labels.iter().map(|l| (*l, "STORAGE")).collect();
    let event = store_open_failed_event(&failures).unwrap();
    assert_eq!(event.tags["stores"].chars().count(), STORES_TAG_MAX);
    assert_eq!(event.tags["store_count"], "20");
}

/// The built event, before the SDK enriches it (so `contexts` is deliberately not
/// asserted here: the SDK adds some). The capturing-transport test below is the real
/// wire check; this pins only what the builder itself sets.
#[test]
fn built_event_has_no_exception_breadcrumbs_or_extra() {
    let event = store_open_failed_event(&[("documents", storage_code())]).unwrap();
    assert!(event.exception.is_empty());
    assert!(event.breadcrumbs.is_empty());
    assert!(event.extra.is_empty());
    assert!(event.logentry.is_none());
}

/// What actually reaches the transport: through the real client, `before_send`
/// redaction and the scope, carries label + code (+ the declared extras) only.
#[test]
fn the_sent_event_carries_label_and_code_only() {
    let code = storage_code();
    let (_, sent) = with_capturing_hub(|| {
        set_install_channel();
        report_store_open_failures(&[("ai_provider_config", code), ("spend", code)]);
    });
    assert_eq!(sent.len(), 1, "one event per launch");
    let events: Vec<&Event> = sent[0]
        .items()
        .filter_map(|item| match item {
            EnvelopeItem::Event(event) => Some(&**event),
            _ => None,
        })
        .collect();
    assert_eq!(events.len(), 1);
    let event = events[0];
    assert_eq!(event.level, Level::Error);
    assert_eq!(event.message.as_deref(), Some("store failed to open"));
    assert_eq!(
        event.fingerprint.iter().map(|s| &**s).collect::<Vec<_>>(),
        ["store-open-failed", "ai_provider_config", code]
    );
    assert_eq!(event.tags["store"], "ai_provider_config");
    assert_eq!(event.tags["error_code"], code);
    // Exactly these tags may appear; `install.channel` is the scope's.
    let mut keys: Vec<&str> = event.tags.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "error_code",
            "install.channel",
            "store",
            "store_count",
            "stores"
        ]
    );
    assert!(event.exception.is_empty());
    assert!(event.user.is_none());
    assert!(event.request.is_none());
    assert!(event.logentry.is_none());
}

#[test]
fn ipc_command_tag_is_set_during_the_call_and_removed_after() {
    with_capturing_hub(|| {
        let seen = with_ipc_command("documents_list", current_tags);
        assert_eq!(seen["ipc.command"], "documents_list");
        assert!(!current_tags().contains_key("ipc.command"));
    });
}

#[test]
fn a_non_identifier_command_is_not_tagged() {
    with_capturing_hub(|| {
        let too_long = "a".repeat(IPC_COMMAND_MAX + 1);
        for bad in [
            "bad command",
            "a;b",
            "plugin:fs|read",
            "",
            too_long.as_str(),
        ] {
            let seen = with_ipc_command(bad, current_tags);
            assert!(!seen.contains_key("ipc.command"), "tagged {bad:?}");
        }
        let at_limit = "a".repeat(IPC_COMMAND_MAX);
        assert!(with_ipc_command(&at_limit, current_tags).contains_key("ipc.command"));
    });
}

#[test]
fn install_channel_tag_is_set_once() {
    with_capturing_hub(|| {
        set_install_channel();
        assert!(current_tags().contains_key("install.channel"));
    });
}

#[test]
fn reporting_without_a_bound_client_is_a_no_op() {
    // No hub bound on this thread's default (no `sentry::init` in tests): must not panic.
    report_store_open_failures(&[("documents", storage_code())]);
    flush(Duration::from_millis(10));
}
