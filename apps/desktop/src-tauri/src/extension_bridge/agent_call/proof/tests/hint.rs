//! Tests for `hint` — the confirmation_required detail text (`proof.rs`).

use super::super::*;

// ── hint — never discloses a value, always names the read surface ─────

#[test]
fn hint_names_the_real_namespaced_read_command() {
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "name",
    };
    let text = hint(source);
    assert!(
        text.contains("agent call documents:documents_list"),
        "{text}"
    );
    assert!(text.contains("name"), "{text}");
}

#[test]
fn hint_never_contains_a_digit_sequence_that_could_be_mistaken_for_a_resolved_value() {
    // Not a full proof of "never leaks the value" (that needs the
    // end-to-end run against a live app — see the manual verification
    // step), but a cheap regression guard: `hint` must be built ONLY
    // from `ProofSource`'s own `'static` field names, never from a
    // resolved `Value`.
    for source in [
        ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        },
        ProofSource::Count {
            read_command: "scrape_list_postings",
        },
    ] {
        let text = hint(source);
        assert!(
            !text.chars().any(|c| c.is_ascii_digit()),
            "hint leaked something numeric: {text}"
        );
    }
}

#[test]
fn hint_falls_back_to_the_bare_command_name_if_somehow_unregistered() {
    // Defensive only — `every_proof_source_read_command_is_a_read_row`
    // (policy.rs) makes this unreachable for a real row, but `hint`
    // itself must still degrade gracefully rather than panic.
    let source = ProofSource::Scalar {
        read_command: "not_a_real_command",
        path: &[],
    };
    assert!(hint(source).contains("not_a_real_command"));
}

/// Issue #1136 turned `applications_list`/`ai_generations_list` from bare
/// arrays into `{items,total,nextCursor}`, and ALL THREE list-shaped proof
/// sources name one of those as their read command (`privacy_reset_app`'s
/// `Count`, `ai_generations_remove`'s `ListMatch`,
/// `ai_generations_remove_bulk`'s `MatchCount`). A hint that still said
/// "its own array length" sent the caller looking for a key that is no
/// longer in the reply, and for a record that may not be on page one DASH so
/// the ceremony's one instruction was wrong for the rows most likely to
/// need it.
#[test]
fn hint_describes_the_paged_reply_shape_for_every_list_shaped_source() {
    let count = hint(ProofSource::Count {
        read_command: "applications_list",
    });
    assert!(
        count.contains("`total`"),
        "a Count proof must name the paged reply's own key: {count}"
    );

    let list_match = hint(ProofSource::ListMatch {
        read_command: "ai_generations_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "jobTitle",
    });
    assert!(
        list_match.contains("`cursor`"),
        "a ListMatch proof must say how to reach a later page: {list_match}"
    );

    let match_count = hint(ProofSource::MatchCount {
        read_command: "ai_generations_list",
        ids_field: &["ids"],
        match_field: "id",
    });
    assert!(
        match_count.contains("`cursor`"),
        "a MatchCount proof must say how to reach a later page: {match_count}"
    );

    // Unchanged guarantee: still built only from `'static` field names,
    // so still incapable of disclosing a resolved value.
    for text in [count, list_match, match_count] {
        assert!(
            !text.chars().any(|c| c.is_ascii_digit()),
            "hint leaked something numeric: {text}"
        );
    }
}
