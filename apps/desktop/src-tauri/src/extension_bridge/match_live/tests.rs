use super::*;

// ── check_autofill_gate (the match.live consent gate) ────────────────────

#[test]
fn check_autofill_gate_refuses_when_opt_in_off() {
    let err = check_autofill_gate(false).unwrap_err();
    assert!(
        err.to_string().contains("Autofill is off"),
        "refusal must carry the shared AUTOFILL_OFF_MESSAGE; got {err}"
    );
}

#[test]
fn check_autofill_gate_allows_when_opt_in_on() {
    assert!(check_autofill_gate(true).is_ok());
}

// ── validate_match_live_request (LOW: gate-first ordering) ───────────────

#[test]
fn validate_match_live_request_prefers_the_autofill_gate_over_emptiness() {
    // Both preconditions fail (opt-in off AND url/html blank) — the gate
    // must win, so an opted-out client ALWAYS sees AUTOFILL_OFF_MESSAGE,
    // consistent with resolve_answers_save/resolve_answers_suggest (both
    // gate before parsing their own payload fields).
    let err = validate_match_live_request(false, "", "").unwrap_err();
    assert!(
        err.to_string().contains("Autofill is off"),
        "the gate failure must win over the emptiness check; got {err}"
    );
}

#[test]
fn validate_match_live_request_reports_emptiness_once_the_gate_passes() {
    let err = validate_match_live_request(true, "", "").unwrap_err();
    assert_eq!(err.to_string(), "url and html are required");
}

#[test]
fn validate_match_live_request_ok_when_both_pass() {
    assert!(
        validate_match_live_request(true, "https://example.com/job/1", "<html></html>").is_ok()
    );
}

// ── match_result_reply ───────────────────────────────────────────────────

fn base_ok() -> MatchLiveOk {
    MatchLiveOk {
        combined: 72.0,
        ats: 60.0,
        gaps: vec!["kubernetes".to_string()],
        resume_name: "My Resume".to_string(),
        salary_posting: None,
        salary_expectation: None,
    }
}

#[test]
fn match_result_reply_carries_ok_payload() {
    let reply = match_result_reply("req-1", Ok(base_ok()));
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::MATCH_RESULT);
    assert_eq!(v["reqId"], "req-1");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["combined"], 72.0);
    assert_eq!(v["payload"]["ats"], 60.0);
    assert_eq!(v["payload"]["resumeName"], "My Resume");
    assert_eq!(v["payload"]["scoreSource"], "keyword");
    assert_eq!(v["payload"]["gaps"][0], "kubernetes");
    assert!(
        v["payload"].get("semantic").is_none(),
        "semantic is never populated by this path"
    );
    assert!(
        v["payload"].get("salary").is_none(),
        "salary must be OMITTED (not null) when no posting range was found"
    );
}

// ── salary facts on the reply (PR3, design decision 5) ────────────────────

#[test]
fn match_result_reply_carries_salary_posting_and_expectation() {
    let mut ok = base_ok();
    ok.salary_posting = Some("$50,000 - $70,000".to_string());
    ok.salary_expectation = Some("€75,000".to_string());
    let reply = match_result_reply("req-2", Ok(ok));
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["salary"]["posting"], "$50,000 - $70,000");
    assert_eq!(v["payload"]["salary"]["expectation"], "€75,000");
}

#[test]
fn match_result_reply_omits_expectation_when_unset_but_keeps_posting() {
    let mut ok = base_ok();
    ok.salary_posting = Some("$50,000 - $70,000".to_string());
    let reply = match_result_reply("req-3", Ok(ok));
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["salary"]["posting"], "$50,000 - $70,000");
    assert!(v["payload"]["salary"].get("expectation").is_none());
}

#[test]
fn match_result_reply_omits_the_whole_salary_field_when_no_posting_range() {
    // Never a lone `expectation` with no posting fact — "two facts side by side" (design
    // decision 5), never one fact alone framed as a comparison target.
    let mut ok = base_ok();
    ok.salary_expectation = Some("€75,000".to_string());
    let reply = match_result_reply("req-4", Ok(ok));
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert!(v["payload"].get("salary").is_none());
}

// ── attach_salary (the "expectation rides only alongside posting" gate) ──

#[test]
fn attach_salary_keeps_both_when_posting_is_found() {
    let ok = attach_salary(
        base_ok(),
        Some("$50,000 - $70,000".to_string()),
        Some("€75,000".to_string()),
    );
    assert_eq!(ok.salary_posting.as_deref(), Some("$50,000 - $70,000"));
    assert_eq!(ok.salary_expectation.as_deref(), Some("€75,000"));
}

#[test]
fn attach_salary_keeps_posting_alone_when_no_expectation_saved() {
    let ok = attach_salary(base_ok(), Some("$50,000 - $70,000".to_string()), None);
    assert_eq!(ok.salary_posting.as_deref(), Some("$50,000 - $70,000"));
    assert!(ok.salary_expectation.is_none());
}

#[test]
fn attach_salary_drops_expectation_when_no_posting_range_was_found() {
    // The invariant: a saved salary expectation must NEVER leave the desktop alone when the
    // posting itself had no extractable range — locks in the gate at its actual source, not
    // just at the reply-serialization layer.
    let ok = attach_salary(base_ok(), None, Some("€75,000".to_string()));
    assert!(
        ok.salary_posting.is_none(),
        "no posting range was found — must stay None"
    );
    assert!(
        ok.salary_expectation.is_none(),
        "expectation must be dropped, never attached without a posting range"
    );
}

#[test]
fn attach_salary_leaves_both_none_when_neither_is_present() {
    let ok = attach_salary(base_ok(), None, None);
    assert!(ok.salary_posting.is_none());
    assert!(ok.salary_expectation.is_none());
}

#[test]
fn match_result_reply_carries_error() {
    let reply = match_result_reply(
        "req-2",
        Err(AppError::Validation(NO_RESUME_MESSAGE.to_string())),
    );
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::MATCH_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["error"], NO_RESUME_MESSAGE);
    assert!(
        v["payload"].get("combined").is_none(),
        "ok:false must never carry success fields"
    );
}
