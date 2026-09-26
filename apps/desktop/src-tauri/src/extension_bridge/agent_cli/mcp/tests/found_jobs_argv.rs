use super::*;
// ── found-jobs tool_argv mapping (MEDIUM fix, review round 2 — this new arm had no
// coverage at all) ───────────────────────────────────────────────────────────────

/// Every field named explicitly (Rust's struct-update `..base` syntax does not
/// exist for enum variants) — mirrors `agent_cli::tests`' own `found_jobs`
/// helper, which this file cannot reuse (a sibling test module, not a
/// descendant).
#[allow(clippy::too_many_arguments)]
fn found_jobs(
    autopilot_id: Option<&str>,
    limit: Option<u64>,
    cursor: Option<&str>,
    min_score: Option<f64>,
    country: Option<&str>,
    remote: Option<bool>,
    applied: Option<bool>,
    query: Option<&str>,
    include_description: bool,
) -> Verb {
    Verb::FoundJobs {
        autopilot_id: autopilot_id.map(str::to_string),
        limit,
        cursor: cursor.map(str::to_string),
        min_score,
        country: country.map(str::to_string),
        remote,
        applied,
        query: query.map(str::to_string),
        include_description,
    }
}

#[test]
fn found_jobs_tool_argv_maps_autopilot_id_limit_and_cursor() {
    let arguments = json!({ "autopilotId": "ap-1", "limit": 10, "cursor": "20" });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(
            Some("ap-1"),
            Some(10),
            Some("20"),
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

#[test]
fn found_jobs_tool_argv_omits_optional_flags_when_absent() {
    let argv = tool_argv(TOOL_FOUND_JOBS, &json!({ "autopilotId": "ap-1" }));
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

/// Issue #1168 — an entirely absent `autopilotId` argument must still round-trip as a
/// VALID spanning traversal, not a usage error.
#[test]
fn found_jobs_tool_argv_omits_autopilot_id_entirely_when_absent() {
    let argv = tool_argv(TOOL_FOUND_JOBS, &json!({}));
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(None, None, None, None, None, None, None, None, false)
    );
}

#[test]
fn found_jobs_tool_argv_maps_every_new_filter() {
    let arguments = json!({
        "autopilotId": "ap-1",
        "minScore": 70,
        "country": "Germany",
        "remote": true,
        "applied": false,
        "query": "engineer",
        "includeDescription": true,
    });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            Some(70.0),
            Some("Germany"),
            Some(true),
            Some(false),
            Some("engineer"),
            true
        )
    );
}

/// HIGH fix, review round 2 — a JSON NUMBER `cursor` (as a real MCP client would send,
/// since the declared schema type is `string` but nothing on the wire enforces that) must
/// still reach `parse_verb` as a string, not be dropped as if the caller had sent nothing.
#[test]
fn found_jobs_tool_argv_forwards_a_numeric_cursor_rather_than_dropping_it() {
    let arguments = json!({ "autopilotId": "ap-1", "cursor": 100 });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            Some("100"),
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

#[test]
fn found_jobs_tool_argv_treats_an_explicit_null_cursor_as_absent() {
    let arguments = json!({ "autopilotId": "ap-1", "cursor": null });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

/// Issue #1137 — `limit: null` was forwarded as the literal string `"null"` and failed
/// `--limit`'s integer parse, while the sibling `cursor` on the same tool already read an
/// explicit null as absent. BOTH `limit` arms are covered: shipping the fix on one of two
/// structurally identical arms guarantees a second issue.
#[test]
fn an_explicit_null_limit_reads_as_absent_on_both_tools_that_take_one() {
    assert_eq!(
        parse_verb(&tool_argv(
            TOOL_FOUND_JOBS,
            &json!({ "autopilotId": "ap-1", "limit": null })
        ))
        .unwrap(),
        found_jobs(
            Some("ap-1"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
    assert_eq!(
        parse_verb(&tool_argv(TOOL_BEST_MATCHES, &json!({ "limit": null }))).unwrap(),
        best_matches(None, None, None)
    );
}

/// The direction the null-filter must NOT regress: a JSON number is still coerced, on the same
/// two arms. A `filter` that swallowed non-strings would pass the test above and break this one.
#[test]
fn a_numeric_limit_still_reaches_parse_verb_on_both_tools() {
    assert_eq!(
        parse_verb(&tool_argv(TOOL_BEST_MATCHES, &json!({ "limit": 7 }))).unwrap(),
        best_matches(Some(7), None, None)
    );
    assert_eq!(
        parse_verb(&tool_argv(
            TOOL_FOUND_JOBS,
            &json!({ "autopilotId": "ap-1", "limit": 7 })
        ))
        .unwrap(),
        found_jobs(
            Some("ap-1"),
            Some(7),
            None,
            None,
            None,
            None,
            None,
            None,
            false
        )
    );
}

/// Issue #1140 — a numeric `confirm` (the shape `ProofSource::Count` proofs really take, e.g. a
/// token count read from `ai_spend_summary`) was dropped by `and_then(Value::as_str)` and
/// answered exactly like "confirm omitted", so a client could loop forever re-reading the same
/// proof and re-sending it the same way.
#[test]
fn a_non_string_confirm_reaches_the_verb_as_its_json_text_rather_than_being_dropped() {
    let arguments = json!({
        "namespace": "documents", "command": "documents_remove", "confirm": 12345,
    });
    let verb = parse_verb(&tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments)).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: Some("12345".to_string()),
        }
    );
}

/// …and the other side of the same fix: an explicit `null` still means "no proof supplied", so a
/// strict-schema client gets the `confirmation_required` hint rather than a mismatch on a value
/// it never sent.
#[test]
fn an_explicit_null_confirm_still_reads_as_absent() {
    let arguments = json!({
        "namespace": "documents", "command": "documents_remove", "confirm": null,
    });
    let verb = parse_verb(&tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments)).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: None,
        }
    );
}

#[test]
fn confirmation_required_result_carries_the_cli_payload_verbatim_plus_one_note() {
    let payload = json!({
        "dispatched": false, "namespace": "ai", "command": "ai_set_provider_key",
        "error": agent_call::ERR_CONFIRMATION_REQUIRED,
        "detail": "read `agent call ai:ai_has_provider_key` and pass its own `has` field as --confirm",
    });
    let result = tool_result(payload.clone(), 4);
    assert_eq!(result["isError"], true);
    let blocks = result["content"].as_array().unwrap();
    assert_eq!(
        blocks[0]["text"],
        payload.to_string(),
        "content[0] must be the CLI payload byte-for-byte"
    );
    assert_eq!(blocks[1]["text"], "exitCode: 4");
    assert!(
        blocks.len() >= 3,
        "a confirmation_required result must carry a third, mapping block"
    );
    assert!(blocks[2]["text"].as_str().unwrap().contains("call-read"));
}
