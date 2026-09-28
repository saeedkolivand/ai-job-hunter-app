//! `resolve_answers_save`'s dedup/cap/refusal paths, then `answers_result_reply`'s wire shape.
//! TOCTOU: `consent::tests`. Dispatch-classification: `frame_advance::tests::classify_write`.
//! The malformed-`auto`-flag guard (no `status.update` counterpart) is in `auto_flag` below.
use super::super::test_support::{app_meta, open_store};
use super::*;
use crate::applications::ApplicationOrigin;

mod auto_flag;

/// Opt-in OFF is a fixed refusal mirroring `resolve_profile`'s exact sentinel
/// — even when a matching Application exists, no answer is ever merged.
#[test]
fn resolve_answers_save_refuses_when_opt_in_off() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let err = resolve_answers_save(
        &store,
        false,
        &json!({ "url": url, "answers": [{ "question": "Why?", "answer": "Because." }] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Autofill is off"), "got: {err}");

    // The refusal must never have written anything.
    assert!(store.get(&id).unwrap().answers.is_empty());
}
/// An empty url is a Validation error — never a panic.
#[test]
fn resolve_answers_save_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err = resolve_answers_save(&store, true, &json!({ "url": "", "answers": [] })).unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A malformed (dangerous-scheme) url hits the distinct invalid-url sentinel
/// — mirrors `resolve_status_update_rejects_malformed_url`.
#[test]
fn resolve_answers_save_rejects_malformed_url() {
    let (_dir, store) = open_store();
    let err = resolve_answers_save(
        &store,
        true,
        &json!({ "url": "javascript:alert(1)", "answers": [] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not a valid"), "got: {err}");
}

/// No Application exists for the url → a clear "import it first" refusal,
/// never a create-on-miss (this verb only appends onto an existing pursuit).
#[test]
fn resolve_answers_save_rejects_when_no_match() {
    let (_dir, store) = open_store();
    let err = resolve_answers_save(
        &store,
        true,
        &json!({ "url": "https://jobs.example.com/posting/none", "answers": [] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("import it first"), "got: {err}");
}

/// An empty `answers` array is a well-formed no-op: `saved: 0, skipped: 0`,
/// never an error.
#[test]
fn resolve_answers_save_handles_empty_list() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-2";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_answers_save(&store, true, &json!({ "url": url, "answers": [] })).unwrap();
    assert_eq!(out.saved, 0);
    assert_eq!(out.skipped, 0);
}

/// The happy path: new questions are added, a re-captured (differently-
/// whitespaced/cased) duplicate of an EXISTING question is skipped, and the
/// existing answer is NEVER overwritten. `title`/`company` ride the reply.
#[test]
fn resolve_answers_save_dedups_against_existing_and_never_overwrites() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-3";
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "seed-1".to_string(),
        question: "Why this role?".to_string(),
        answer: "Original answer".to_string(),
    }];
    let id = store
        .upsert_for_origin(url, "linkedin", &meta, ApplicationOrigin::Saved, None)
        .unwrap();

    let out = resolve_answers_save(
        &store,
        true,
        &json!({
            "url": url,
            "answers": [
                { "question": "why   THIS role?", "answer": "A different answer that must be dropped" },
                { "question": "What's your salary expectation?", "answer": "100k" },
            ],
        }),
    )
    .unwrap();
    assert_eq!(out.saved, 1, "only the genuinely new question is added");
    assert_eq!(out.skipped, 1, "the re-captured duplicate is dedup-dropped");
    assert_eq!(out.title.as_deref(), Some("Backend Engineer"));
    assert_eq!(out.company.as_deref(), Some("Acme"));

    let row = store.get(&id).unwrap();
    assert_eq!(row.answers.len(), 2);
    let original = row
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .expect("the seeded answer must survive");
    assert_eq!(
        original.answer, "Original answer",
        "an existing answer must NEVER be overwritten by a re-capture"
    );
    assert!(row
        .answers
        .iter()
        .any(|a| a.question == "What's your salary expectation?" && a.answer == "100k"));
}

/// Oversized question/answer text is clamped (char-boundary safe), not
/// dropped — mirrors `applications::clamp_job_description`'s discipline.
#[test]
fn resolve_answers_save_clamps_oversized_question_and_answer_bytes() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-4";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let huge_question = "q".repeat(5_000);
    let huge_answer = "a".repeat(20_000);
    let out = resolve_answers_save(
        &store,
        true,
        &json!({
            "url": url,
            "answers": [{ "question": huge_question, "answer": huge_answer }],
        }),
    )
    .unwrap();
    assert_eq!(out.saved, 1);

    let row = store.get(&id).unwrap();
    let saved = &row.answers[0];
    assert!(saved.question.len() <= 1_000, "question must be clamped");
    assert!(saved.answer.len() <= 8_000, "answer must be clamped");
}

/// More than 50 entries in one call are capped — extras are silently
/// dropped, never rejected outright, and MUST still be reflected in
/// `skipped` (not silently vanish from both counts).
#[test]
fn resolve_answers_save_caps_at_max_answers_per_call() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-5";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Principal Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let answers: Vec<serde_json::Value> = (0..75)
        .map(|i| json!({ "question": format!("Question {i}?"), "answer": format!("Answer {i}") }))
        .collect();
    let out =
        resolve_answers_save(&store, true, &json!({ "url": url, "answers": answers })).unwrap();
    assert_eq!(out.saved, 50, "extras beyond the 50-entry cap are dropped");
    assert_eq!(
        out.skipped, 25,
        "the 25 entries beyond the per-call cap must reconcile into skipped, not vanish"
    );
}

/// The STORE-level 500-answer cap (not the 50-per-call cap above) is what
/// limits this call: a well-under-50 batch still can't push the total past
/// [`crate::applications::MAX_TOTAL_ANSWERS`], and the overflow must land in
/// `skipped` exactly like the per-call cap does.
#[test]
fn resolve_answers_save_reflects_store_level_total_cap_in_skipped() {
    use crate::applications::MAX_TOTAL_ANSWERS;

    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-store-cap";
    let mut meta = app_meta("Acme", "Cap Test Engineer");
    // Seed to exactly (cap - 2) existing distinct answers via the store
    // directly — bypassing the 50-per-call wire cap entirely.
    meta.answers = (0..MAX_TOTAL_ANSWERS - 2)
        .map(|i| ApplicationAnswer {
            id: format!("seed-{i}"),
            question: format!("Existing question {i}?"),
            answer: format!("Existing answer {i}"),
        })
        .collect();
    store
        .upsert_for_origin(url, "linkedin", &meta, ApplicationOrigin::Saved, None)
        .unwrap();

    // 5 new distinct questions — well under MAX_ANSWERS_PER_CALL (50), so the
    // per-call cap never triggers; only the store's cumulative 500 cap does.
    let answers: Vec<serde_json::Value> = (0..5)
        .map(|i| json!({ "question": format!("New question {i}?"), "answer": format!("New answer {i}") }))
        .collect();
    let out =
        resolve_answers_save(&store, true, &json!({ "url": url, "answers": answers })).unwrap();
    assert_eq!(
        out.saved, 2,
        "only enough new answers to reach the store cap are saved"
    );
    assert_eq!(
        out.skipped, 3,
        "the 3 answers that didn't fit under the store's total cap must show up as skipped"
    );
}

/// `answers_result_reply` builds a well-formed success envelope carrying the
/// counts + title/company (mirrors `status_result_reply_carries_ok_true_and_fields`).
#[test]
fn answers_result_reply_carries_ok_true_and_counts() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-6";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Reply Test Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let outcome = resolve_answers_save(
        &store,
        true,
        &json!({ "url": url, "answers": [{ "question": "Why?", "answer": "Because." }] }),
    );
    let reply = answers_result_reply("req-13", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::ANSWERS_RESULT);
    assert_eq!(v["reqId"], "req-13");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["saved"], 1);
    assert_eq!(v["payload"]["skipped"], 0);
    assert_eq!(v["payload"]["title"], "Reply Test Engineer");
    assert!(v["payload"]["applicationId"].is_string());
}

/// UNLIKE `applied.result`, an `answers.save` failure carries a fixed,
/// user-facing `error` string — this verb's errors are surfaced to the user.
#[test]
fn answers_result_reply_carries_user_facing_error() {
    let (_dir, store) = open_store();
    let outcome = resolve_answers_save(
        &store,
        true,
        &json!({ "url": "https://jobs.example.com/posting/none", "answers": [] }),
    );
    let reply = answers_result_reply("req-14", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::ANSWERS_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("import it first"));
}
