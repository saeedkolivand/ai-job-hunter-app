use super::*;

// ── payload shape / non-contamination ─────────────────────────────────────

/// The one line that makes any of this reachable.
///
/// `#[tauri::command] match_resume` needs an `AppHandle`, so no test in this
/// crate can call it — delete `constraints::attach` from it and every test
/// above stays green while the feature silently disappears from the app.
/// (Verified: that mutation survived the whole suite before this test
/// existed.) A compile-time source scan is the cheapest honest pin, and the
/// same technique `tests/architecture.rs` already uses for invariants a
/// linked build cannot see.
///
/// The call is pinned as the function's TAIL EXPRESSION — the exact leading
/// indentation, then the closing brace on the next line. A weaker
/// `contains("constraints::attach")` would stay green for
/// `let _ = constraints::attach(…);` or a commented-out call, both of which
/// keep the text while dropping the feature; the discard form is rejected
/// explicitly too, so the failure names the mistake.
#[test]
fn the_match_resume_command_returns_the_attached_report() {
    const COMMAND_SRC: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/match_resume.rs"
    ));
    // Matched as a WHOLE LINE, which is what makes the weak forms fail: a
    // commented-out call carries a `//` prefix, and the discard form starts
    // `let _ =`, so neither is equal to this.
    const CALL: &str = "    constraints::attach(&app, &posting_facts, scored)";
    assert!(
        COMMAND_SRC.lines().any(|line| line == CALL),
        "commands::match_resume must RETURN score_one's result through \
             constraints::attach — without that call the hard-constraint pass \
             never reaches the payload, and nothing else fails"
    );
    // Nothing here needs to check that the call is the function's LAST
    // expression: `CALL` carries no trailing semicolon, and a `Value`-typed
    // expression statement that is NOT the tail does not compile. The
    // compiler owns that half; this test owns "the line is still there".
    // (Deliberately no closing-brace literal — the egress inventory's
    // stripper counts braces naively and cannot see into string literals, so
    // an unbalanced one here truncates the region it strips from this file.)
    assert!(
        !COMMAND_SRC.contains("let _ = constraints::attach"),
        "discarding the attached report drops the feature while keeping the call"
    );
}

/// The scoring fields must come out the other side byte-identical, and the
/// verdict must arrive as its own sibling field — never folded in.
#[test]
fn attaching_a_report_leaves_every_scoring_field_untouched() {
    let scored = json!({
        "resumeId": "r1",
        "jobId": "j1",
        "ats": 40.0,
        "semantic": 80.0,
        "combined": 64.0,
        "gaps": ["kubernetes"],
        "scoreSource": "combined",
    });
    let checks = evaluate(
        &posting(Some("Austin, TX"), false),
        &candidate(Some("Berlin")),
    );
    let merged = merge(scored.clone(), || checks.clone());
    // The verdict arrived as its own sibling field, not folded in anywhere —
    // and the far-away posting reads `unknown`, not `notMet`.
    assert_eq!(
        merged["constraints"],
        json!({ "checks": [{
                "id": "preferredLocation",
                "status": "unknown",
                "posting": "Austin, TX",
                "candidate": "Berlin",
            }] })
    );
    // The number did not move: 64, the absolute value 0.6 × 80 + 0.4 × 40
    // produces.
    assert_eq!(merged["combined"], json!(64.0));
    assert_eq!(merged["ats"], json!(40.0));
    assert_eq!(merged["semantic"], json!(80.0));
    assert_eq!(merged["scoreSource"], json!("combined"));
    assert_eq!(merged["gaps"], json!(["kubernetes"]));
    // And every pre-existing field is byte-identical: the merge adds one key
    // and rewrites none.
    let mut without_constraints = merged.clone();
    without_constraints
        .as_object_mut()
        .unwrap()
        .remove("constraints");
    assert_eq!(without_constraints, scored);
}

#[test]
fn an_error_result_is_returned_untouched_and_costs_no_evaluation() {
    let err = json!({ "error": "job not found in cache: j1" });
    let mut evaluated = false;
    let out = merge(err.clone(), || {
        evaluated = true;
        Vec::new()
    });
    assert_eq!(out, err);
    assert!(out.get("constraints").is_none());
    assert!(
        !evaluated,
        "an error object has no posting to state anything — don't even look"
    );
}

#[test]
fn the_wire_shape_is_camel_case_with_absent_evidence_omitted() {
    let checks = evaluate(&posting(None, false), &candidate(Some("Berlin")));
    assert_eq!(
        serde_json::to_value(&checks).unwrap(),
        json!([{ "id": "preferredLocation", "status": "unknown", "candidate": "Berlin" }])
    );
    let met = evaluate(
        &posting(Some("Berlin, Germany"), false),
        &candidate(Some("Berlin")),
    );
    assert_eq!(
        serde_json::to_value(&met).unwrap(),
        json!([{
            "id": "preferredLocation",
            "status": "met",
            "posting": "Berlin, Germany",
            "candidate": "Berlin",
        }])
    );
}

#[test]
fn posting_facts_read_the_flattened_remote_flag_and_location() {
    let facts = posting_facts_from_value(&json!({
        "id": "j1",
        "title": "Engineer",
        "location": "Austin, TX",
        // `JobPosting::extra` is `#[serde(flatten)]`, so board metadata sits
        // at the top level of the cached value, not under `extra`.
        "remote": true,
    }));
    assert_eq!(
        facts,
        PostingFacts {
            location: Some("Austin, TX".to_string()),
            board_remote: true,
        }
    );
    // A posting with neither reads as "states nothing", not as false data.
    assert_eq!(
        posting_facts_from_value(&json!({ "id": "j2" })),
        PostingFacts {
            location: None,
            board_remote: false,
        }
    );
}

#[test]
fn evidence_is_byte_capped_on_a_char_boundary() {
    let long = "ü".repeat(400); // 800 bytes
    let checks = evaluate(&posting(Some(&long), false), &candidate(Some(&long)));
    let check = only(&checks);
    let posted = check.posting().unwrap();
    assert_eq!(posted.len(), MAX_EVIDENCE_BYTES);
    assert_eq!(posted.chars().count(), MAX_EVIDENCE_BYTES / 2);
    assert_eq!(check.candidate().unwrap().len(), MAX_EVIDENCE_BYTES);
}
