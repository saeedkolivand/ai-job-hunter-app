//! `ImportOutcome` serde-contract tests plus the two small helper-fn tests
//! (`import_cookies` unknown-board path, `path_with_suffix`).

use super::super::decrypt::path_with_suffix;
use super::super::*;

#[test]
fn outcome_serializes_pascal_case() {
    assert_eq!(
        serde_json::to_value(ImportOutcome::BrowserNotFound).unwrap(),
        serde_json::json!("BrowserNotFound")
    );
    assert_eq!(
        serde_json::to_value(ImportOutcome::NoSession).unwrap(),
        serde_json::json!("NoSession")
    );
    // Tuple variant serializes as { "Imported": n } — the command layer maps it
    // to a flat shape, but PascalCase naming is what we assert here.
    assert_eq!(
        serde_json::to_value(ImportOutcome::Imported(3)).unwrap(),
        serde_json::json!({ "Imported": 3 })
    );
}

#[test]
fn import_unknown_board_is_no_session() {
    let tmp = std::env::temp_dir();
    let outcome = import_cookies(&tmp, "definitely-not-a-board").unwrap();
    assert_eq!(outcome, ImportOutcome::NoSession);
}

#[test]
fn path_with_suffix_appends() {
    let p = std::path::Path::new("/x/Cookies");
    assert_eq!(
        path_with_suffix(p, "-wal"),
        std::path::PathBuf::from("/x/Cookies-wal")
    );
}

// ── Gap 4: ImportOutcome serde completeness ───────────────────────────────────

/// `Undecryptable` was missing from the existing serde test. Assert the exact
/// JSON string the frontend contract expects.
#[test]
fn outcome_undecryptable_serializes_pascal_case() {
    assert_eq!(
        serde_json::to_value(ImportOutcome::Undecryptable).unwrap(),
        serde_json::json!("Undecryptable")
    );
}

/// All four variants together — one table-driven assertion to catch any future
/// rename that breaks the IPC contract.
#[test]
fn outcome_all_variants_ipc_contract() {
    use serde_json::json;

    let cases: &[(ImportOutcome, serde_json::Value)] = &[
        (ImportOutcome::Imported(0), json!({ "Imported": 0 })),
        (ImportOutcome::Imported(42), json!({ "Imported": 42 })),
        (ImportOutcome::NoSession, json!("NoSession")),
        (ImportOutcome::Undecryptable, json!("Undecryptable")),
        (ImportOutcome::BrowserNotFound, json!("BrowserNotFound")),
    ];

    for (variant, expected) in cases {
        let got = serde_json::to_value(variant).unwrap();
        assert_eq!(got, *expected, "serde mismatch for variant");
    }
}
