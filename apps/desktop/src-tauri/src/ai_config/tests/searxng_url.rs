//! #1515: the SearXNG-compatible search URL persisted beside the provider config.

use serde_json::json;

use super::support::new_store;
use super::*;

#[test]
fn unset_by_default_and_set_then_cleared() {
    let (_dir, store) = new_store();
    assert_eq!(store.searxng_url(), None);

    store
        .set_searxng_url(Some(" http://localhost:8888/ "))
        .unwrap();
    assert_eq!(
        store.searxng_url().as_deref(),
        Some("http://localhost:8888")
    );

    store.set_searxng_url(Some("   ")).unwrap();
    assert_eq!(store.searxng_url(), None, "blank clears");

    store
        .set_searxng_url(Some("http://localhost:8888"))
        .unwrap();
    store.set_searxng_url(None).unwrap();
    assert_eq!(store.searxng_url(), None);
}

/// Mutation check: skip the validation in `set_searxng_url` and the stored value changes.
#[test]
fn an_invalid_url_is_rejected_and_the_stored_one_is_kept() {
    let (_dir, store) = new_store();
    store
        .set_searxng_url(Some("https://search.example.org"))
        .unwrap();
    for bad in [
        "ftp://x",
        "javascript:alert(1)",
        "not a url",
        "http://169.254.169.254",
    ] {
        assert!(store.set_searxng_url(Some(bad)).is_err(), "{bad}");
    }
    assert_eq!(
        store.searxng_url().as_deref(),
        Some("https://search.example.org")
    );
}

#[test]
fn backup_round_trips_and_a_factory_reset_clears_it() {
    let (_dir, store) = new_store();
    store
        .set_searxng_url(Some("http://localhost:8888"))
        .unwrap();
    let exported = store.export();
    assert_eq!(exported["searxngUrl"], "http://localhost:8888");

    let (_dir2, restored) = new_store();
    restored.import(&exported).unwrap();
    assert_eq!(
        restored.searxng_url().as_deref(),
        Some("http://localhost:8888")
    );

    store.clear();
    assert_eq!(store.searxng_url(), None);
}

/// A tampered bundle cannot plant an egress URL: an invalid one is dropped on restore.
#[test]
fn a_tampered_backup_cannot_restore_an_invalid_url() {
    let (_dir, store) = new_store();
    store
        .set_searxng_url(Some("http://localhost:8888"))
        .unwrap();
    store
        .import(&json!({ "providers": {}, "searxngUrl": "file:///etc/passwd" }))
        .unwrap();
    assert_eq!(
        store.searxng_url(),
        None,
        "replace semantics, bad value dropped"
    );
}

/// A stored value that fails today's rules (written before they tightened, or planted by a
/// restore) is never returned: not to the egress path, not to `ai_search_settings`.
#[test]
fn an_invalid_stored_value_reads_as_unset() {
    let (_dir, store) = new_store();
    for bad in [
        "http://search.example.org",
        "https://user:pw@host.example.org",
        "ftp://x",
    ] {
        AiConfigStore::write_searxng_url_conn(&store.conn.lock(), Some(bad)).unwrap();
        assert_eq!(store.searxng_url(), None, "{bad}");
        assert!(store.export().get("searxng_url").is_none());
    }
}

#[test]
fn credentials_in_the_url_are_refused() {
    let (_dir, store) = new_store();
    assert!(store
        .set_searxng_url(Some("https://u:p@search.example.org"))
        .is_err());
    assert!(store
        .set_searxng_url(Some("https://u@search.example.org"))
        .is_err());
    assert_eq!(store.searxng_url(), None);
}

#[test]
fn an_unconfigured_export_omits_the_field() {
    let (_dir, store) = new_store();
    assert!(store.export().get("searxngUrl").is_none());
}
