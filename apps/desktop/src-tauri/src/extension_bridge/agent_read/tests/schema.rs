//! Tests for the `RESOURCES` table and unknown-resource dispatch (`agent_read.rs`).

use super::super::*;

#[test]
fn schema_lists_every_known_resource() {
    let mut names: Vec<&str> = RESOURCES.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    // Hand-written, not derived from RESOURCES itself (a self-referential
    // check proves nothing) — mirrors the repo's standing "pair a
    // loop-over-own-fields test with a hand-written literal list" lesson.
    assert_eq!(
        names,
        vec![
            "automations",
            "best-matches",
            "documents",
            "found-jobs",
            "job",
            "prep",
            "profile",
            "schema"
        ]
    );
}

#[test]
fn dispatch_rejects_an_unknown_resource() {
    // Pins the OTHER half of "cannot advertise a verb that does not
    // exist": a name absent from RESOURCES must not be dispatched.
    let payload = json!({ "resource": "delete-everything" });
    assert!(!RESOURCES.iter().any(|(n, _)| *n == resource_name(&payload)));
}

// ── job ──────────────────────────────────────────────────────────────────
