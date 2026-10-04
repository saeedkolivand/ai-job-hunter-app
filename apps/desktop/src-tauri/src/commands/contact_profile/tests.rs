use tempfile::TempDir;

use super::*;

#[test]
fn clamp_lang_passes_a_short_code_through_unchanged() {
    assert_eq!(clamp_lang("de"), "de");
    assert_eq!(clamp_lang("en-US"), "en-US");
}

#[test]
fn clamp_lang_truncates_an_oversized_value() {
    let huge = "x".repeat(1000);
    assert_eq!(clamp_lang(&huge).len(), 16);
}

/// A real, isolated store (temp SQLite file) — not a mock — so the
/// managed-branch tests below exercise the actual `ContactProfileStore`
/// read/write path, not a stand-in. `TempDir` must outlive the store (it
/// deletes on drop), hence returning both.
fn store() -> (TempDir, ContactProfileStore) {
    let dir = TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

// ── contact_profile_get_inner ───────────────────────────────────────────

#[test]
fn get_inner_degrades_to_default_profile_when_store_unmanaged() {
    assert_eq!(
        contact_profile_get_inner(None),
        json!(ContactProfile::default())
    );
}

#[test]
fn get_inner_returns_the_stored_profile_when_managed() {
    let (_dir, store) = store();
    store
        .set(&ContactProfile {
            full_name: Some("Jane Doe".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        contact_profile_get_inner(Some(&store))["fullName"],
        "Jane Doe"
    );
}

// ── contact_profile_set_inner ───────────────────────────────────────────

/// The HIGH-1 repro: an unmanaged store must REJECT (not silently
/// "succeed" with an in-band error field nobody reads).
#[test]
fn set_inner_rejects_when_store_unmanaged() {
    let err = contact_profile_set_inner(None, json!({ "fullName": "Jane Doe" })).unwrap_err();
    assert!(matches!(err, AppError::Storage(_)), "{err:?}");
}

#[test]
fn set_inner_persists_and_reports_success_when_managed() {
    let (_dir, store) = store();
    let result = contact_profile_set_inner(Some(&store), json!({ "fullName": "Jane Doe" }));
    assert_eq!(result.unwrap(), json!({ "success": true }));
    assert_eq!(store.get().full_name.as_deref(), Some("Jane Doe"));
}

#[test]
fn set_inner_rejects_an_invalid_payload_even_when_managed() {
    let (_dir, store) = store();
    let err =
        contact_profile_set_inner(Some(&store), json!("not a contact profile object")).unwrap_err();
    assert!(matches!(err, AppError::Parse(_)), "{err:?}");
}

// ── contact_profile_header_line_inner ───────────────────────────────────

#[test]
fn header_line_inner_degrades_to_empty_string_when_store_unmanaged() {
    assert_eq!(contact_profile_header_line_inner(None, "en"), "");
}

#[test]
fn header_line_inner_returns_the_localized_header_when_managed() {
    let (_dir, store) = store();
    store
        .set(&ContactProfile {
            email: Some("jane@example.com".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        contact_profile_header_line_inner(Some(&store), "en"),
        "jane@example.com"
    );
}
