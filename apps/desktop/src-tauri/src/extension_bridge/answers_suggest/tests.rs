//! `resolve_answers_suggest` (matching + Task #30's synthetic salary row) and
//! `answers_suggest_reply`'s wire shape. See `matcher::tests`/`frame_advance::tests` for the rest.
use super::super::test_support::{app_meta, open_store};
use super::*;
use crate::ai_generations::ApplicationAnswer;
use crate::applications::ApplicationOrigin;

/// Opt-in OFF is a fixed refusal mirroring `resolve_answers_save`'s exact
/// sentinel — even with matching answers available, nothing is ever returned.
#[test]
fn resolve_answers_suggest_refuses_when_opt_in_off() {
    let (_dir, store) = open_store();
    let err = resolve_answers_suggest(
        &store,
        false,
        None,
        &json!({ "questions": ["Why this role?"] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Autofill is off"), "got: {err}");
}

/// No/empty `questions` is a well-formed no-op — never an error.
#[test]
fn resolve_answers_suggest_returns_empty_list_for_no_questions() {
    let (_dir, store) = open_store();
    let out = resolve_answers_suggest(&store, true, None, &json!({ "questions": [] })).unwrap();
    assert!(out.is_empty());
}

/// Aggregates across MULTIPLE applications (not just one) via the real store —
/// this is the read-only integration point that stands in for a dedicated
/// store method (see the module doc: `applications/mod.rs` is at the R8 cap).
#[test]
fn resolve_answers_suggest_matches_across_multiple_applications() {
    let (_dir, store) = open_store();
    let mut meta_a = app_meta("Acme", "Backend Engineer");
    meta_a.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "Why do you want to work here?".to_string(),
        answer: "Because I love building things.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-a",
            "linkedin",
            &meta_a,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let mut meta_b = app_meta("Globex", "QA Engineer");
    meta_b.answers = vec![ApplicationAnswer {
        id: "b1".to_string(),
        question: "What is your notice period?".to_string(),
        answer: "Two weeks.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-b",
            "linkedin",
            &meta_b,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_answers_suggest(
        &store,
        true,
        None,
        &json!({ "questions": ["What is your notice period?"] }),
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Two weeks.");
    assert_eq!(out[0].source_company.as_deref(), Some("Globex"));
}

/// Read-only proof: `resolve_answers_suggest` must never mutate the store — a
/// `list()` snapshot taken before and after a real (matching) call is
/// byte-identical.
#[test]
fn resolve_answers_suggest_never_mutates_the_store() {
    let (_dir, store) = open_store();
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "Why do you want to work here?".to_string(),
        answer: "Because I love building things.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-readonly",
            "linkedin",
            &meta,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let before = serde_json::to_value(store.list()).unwrap();
    let _ = resolve_answers_suggest(
        &store,
        true,
        None,
        &json!({ "questions": ["Why do you want to work here?"] }),
    )
    .unwrap();
    let after = serde_json::to_value(store.list()).unwrap();

    assert_eq!(before, after, "answers.suggest must never mutate the store");
}

/// Synthetic salary row (Task #30): a salary-shaped question with no stored answer still gets a suggestion.
#[test]
fn resolve_answers_suggest_synthesizes_salary_row_when_no_stored_match() {
    let (_dir, store) = open_store();
    let out = resolve_answers_suggest(
        &store,
        true,
        Some("€75,000"),
        &json!({ "questions": ["What are your salary expectations?"] }),
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "€75,000");
    assert_eq!(out[0].source_company.as_deref(), Some("Saved expectation"));
    assert_eq!(out[0].score, 1.0);
    assert!(out[0].salary);
}

/// Same synthetic fill for a German salary-shaped question — proves the
/// synthetic path shares the same (now DACH-aware) `is_salary_question`.
#[test]
fn resolve_answers_suggest_synthesizes_salary_row_for_german_question() {
    let (_dir, store) = open_store();
    let out = resolve_answers_suggest(
        &store,
        true,
        Some("80.000 EUR"),
        &json!({ "questions": ["Was ist Ihre Gehaltsvorstellung?"] }),
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "80.000 EUR");
    assert!(out[0].salary);
}

/// No backend salary expectation (renderer-only value not yet synced, or
/// blank) → no synthetic row, and the salary question is simply absent from
/// the reply (never an error).
#[test]
fn resolve_answers_suggest_no_synthetic_row_without_expectation() {
    let (_dir, store) = open_store();
    let out = resolve_answers_suggest(
        &store,
        true,
        None,
        &json!({ "questions": ["What are your salary expectations?"] }),
    )
    .unwrap();
    assert!(out.is_empty());

    // A blank/whitespace-only expectation is treated the same as absent.
    let out_blank = resolve_answers_suggest(
        &store,
        true,
        Some("   "),
        &json!({ "questions": ["What are your salary expectations?"] }),
    )
    .unwrap();
    assert!(out_blank.is_empty());
}

/// A REAL stored salary answer wins over the synthetic fill — the synthetic
/// row only fills a gap `match_questions` left unanswered, it never competes
/// with (or duplicates alongside) a genuine stored match for the SAME
/// question.
#[test]
fn resolve_answers_suggest_stored_answer_wins_over_synthetic() {
    let (_dir, store) = open_store();
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "What is your expected salary?".to_string(),
        answer: "$120,000, negotiable.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/salary-wins",
            "linkedin",
            &meta,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_answers_suggest(
        &store,
        true,
        Some("€75,000"),
        &json!({ "questions": ["What is your expected salary?"] }),
    )
    .unwrap();
    assert_eq!(
        out.len(),
        1,
        "the stored answer must win — never a second, competing synthetic row"
    );
    assert_eq!(out[0].answer, "$120,000, negotiable.");
    assert_eq!(out[0].source_company.as_deref(), Some("Acme"));
}

/// The synthetic row only fills questions the stored match left UNANSWERED —
/// a non-salary question with its own stored match, plus a salary question
/// with none, both come back (one real, one synthetic).
#[test]
fn resolve_answers_suggest_synthetic_only_fills_the_gap_alongside_a_real_match() {
    let (_dir, store) = open_store();
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "Why do you want to work here?".to_string(),
        answer: "Because I love building things.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/salary-gap",
            "linkedin",
            &meta,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_answers_suggest(
        &store,
        true,
        Some("€75,000"),
        &json!({
            "questions": [
                "Why do you want to work here?",
                "What are your salary expectations?"
            ]
        }),
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert!(out
        .iter()
        .any(|s| s.answer == "Because I love building things." && !s.salary));
    assert!(out.iter().any(|s| s.answer == "€75,000" && s.salary));
}

/// `answers_suggest_reply` carries `ok:true` + the suggestions array.
#[test]
fn answers_suggest_reply_carries_type_and_req_id_on_success() {
    let reply = answers_suggest_reply(
        "req-1",
        Ok(vec![Suggestion {
            question: "Why this role?".to_string(),
            answer: "Because I love it.".to_string(),
            source_company: Some("Acme".to_string()),
            source_title: Some("Backend Engineer".to_string()),
            source_question: "Why do you want to work here?".to_string(),
            score: 0.8,
            salary: false,
        }]),
    );
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::ANSWERS_SUGGEST_RESULT);
    assert_eq!(v["reqId"], "req-1");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["suggestions"][0]["question"], "Why this role?");
    assert_eq!(v["payload"]["suggestions"][0]["sourceCompany"], "Acme");
    assert_eq!(
        v["payload"]["suggestions"][0]["sourceQuestion"],
        "Why do you want to work here?"
    );
    assert_eq!(v["payload"]["suggestions"][0]["salary"], false);
}

/// `answers_suggest_reply` carries `ok:false` + the refusal error.
#[test]
fn answers_suggest_reply_carries_refusal_error() {
    let reply = answers_suggest_reply(
        "req-2",
        Err(crate::error::AppError::Validation(
            crate::extension_bridge::AUTOFILL_OFF_MESSAGE.to_string(),
        )),
    );
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("Autofill is off"));
}
