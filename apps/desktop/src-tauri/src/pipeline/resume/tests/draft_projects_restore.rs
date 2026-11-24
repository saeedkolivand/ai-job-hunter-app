//! #1549: a draft that dropped Projects gets the source's own section back.

use super::super::stages::apply_projects_normalization;

/// The model omitted Projects although the source has one — the source's own
/// section is appended verbatim and a warning is recorded.
#[test]
fn restores_a_dropped_projects_section() {
    let source = "SUMMARY\n\nBuilder.\n\nPROJECTS\n\nLedger CLI - https://github.com/janedoe/ledger\nA bookkeeping tool.\n";
    let draft = "SUMMARY\n\nBuilder of tools.\n";
    let (text, artifact) = apply_projects_normalization(source, draft.to_string());
    assert!(text.starts_with("SUMMARY\n\nBuilder of tools."));
    assert!(text.contains(
        "PROJECTS\n\nLedger CLI - https://github.com/janedoe/ledger\nA bookkeeping tool."
    ));
    assert_eq!(artifact["warnings"][0], "projects_restored_from_source");
    assert_eq!(artifact["chars"], text.chars().count());
}

/// A source with no Projects section: nothing is invented, no warning.
#[test]
fn adds_nothing_when_source_has_no_projects() {
    let draft = "SUMMARY\n\nBuilder of tools.\n";
    let (text, artifact) = apply_projects_normalization("SUMMARY\n\nBuilder.\n", draft.to_string());
    assert_eq!(text, draft);
    assert!(artifact.get("warnings").is_none());
}
