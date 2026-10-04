use super::*;

// ── Boundary validation ──────────────────────────────────────────────────────

#[test]
fn an_empty_query_is_refused() {
    // The command trims first, so a whitespace-only query arrives here empty.
    let err = validate("", &corpus()).unwrap_err();
    assert!(
        matches!(err, AppError::Validation(_)),
        "expected a validation error, got {err:?}"
    );
}

#[test]
fn a_query_past_the_cap_is_refused() {
    let long = "x".repeat(QUERY_MAX_CHARS + 1);
    assert!(validate(&long, &corpus()).is_err());
    // The cap itself is inclusive — exactly at the limit must still pass.
    assert!(validate(&"x".repeat(QUERY_MAX_CHARS), &corpus()).is_ok());
}

#[test]
fn the_query_cap_counts_chars_not_bytes() {
    // A multi-byte query at the char cap must not be refused for its byte
    // length: the Zod cap the renderer enforces is a CHAR cap, so a
    // byte-counting check here would refuse a legitimate German or Japanese
    // question that the schema accepted.
    let multibyte = "ü".repeat(QUERY_MAX_CHARS);
    assert!(
        multibyte.len() > QUERY_MAX_CHARS,
        "fixture must be multi-byte"
    );
    assert!(validate(&multibyte, &corpus()).is_ok());
}

#[test]
fn an_empty_entry_list_is_refused() {
    assert!(validate("how do i export", &[]).is_err());
}

#[test]
fn too_many_entries_are_refused() {
    let many: Vec<HelpSearchRequestEntry> = (0..=ENTRIES_MAX)
        .map(|i| entry(&format!("s.e{i}"), "title", "body"))
        .collect();
    assert!(many.len() > ENTRIES_MAX);
    assert!(validate("export", &many).is_err());
    assert!(validate("export", &many[..ENTRIES_MAX]).is_ok());
}

#[test]
fn an_oversized_entry_body_is_refused() {
    let entries = vec![entry("s.e", "title", &"x".repeat(ENTRY_BODY_MAX_CHARS + 1))];
    assert!(validate("export", &entries).is_err());
}

#[test]
fn an_oversized_entry_title_is_refused() {
    let entries = vec![entry("s.e", &"x".repeat(ENTRY_TITLE_MAX_CHARS + 1), "body")];
    assert!(validate("export", &entries).is_err());
}

#[test]
fn an_oversized_or_empty_entry_id_is_refused() {
    assert!(validate("export", &[entry("", "title", "body")]).is_err());
    assert!(validate(
        "export",
        &[entry(&"a".repeat(ENTRY_ID_MAX_CHARS + 1), "title", "body")]
    )
    .is_err());
}

#[test]
fn an_entry_id_outside_the_schemas_charset_is_refused() {
    // Ids are echoed straight back to the caller. Anything that could not
    // have come from a translation leaf path is refused rather than
    // round-tripped — the schema's own `^[A-Za-z0-9_.-]+$`.
    for bad in ["a b", "a/b", "a<b", "señor"] {
        assert!(
            validate("export", &[entry(bad, "title", "body")]).is_err(),
            "id `{bad}` should be refused"
        );
    }
    assert!(validate(
        "export",
        &[entry("aiGenerateQuestions.export-doc_1", "t", "b")]
    )
    .is_ok());
}

#[test]
fn a_help_prefixed_query_id_is_accepted_and_every_other_shape_is_refused() {
    // Optional: an agent-CLI caller that sends no id gets no cancellation,
    // not a refusal.
    assert!(validate_query_id(None).is_ok());
    assert!(validate_query_id(Some("help-0d3f")).is_ok());
    assert!(
        validate_query_id(Some(&format!("{QUERY_ID_PREFIX}{}", "x".repeat(59)))).is_ok(),
        "exactly at the cap must pass — the cap is inclusive"
    );

    for bad in [
        // The postings search's own prefix: a shared id space where either
        // feature could name the other's live search is exactly what the
        // prefixes exist to prevent.
        "search-0d3f",
        // A Rust-minted job id — the collision that would replace a live
        // scrape's token and then delete its slot.
        "job-0d3f",
        "run-0d3f",
        "",
        "0d3f",
        "HELP-0d3f",
    ] {
        assert!(
            matches!(validate_query_id(Some(bad)), Err(AppError::Validation(_))),
            "`{bad}` must be refused at the boundary"
        );
    }
    // 65 chars: one past the cap.
    let too_long = format!("{QUERY_ID_PREFIX}{}", "x".repeat(60));
    assert_eq!(too_long.chars().count(), QUERY_ID_MAX_CHARS + 1);
    assert!(validate_query_id(Some(&too_long)).is_err());
}
