use super::{support::*, *};

// ── merge_answers (extension bridge `answers.save`'s store-write boundary) ────
// APPEND-only dedup merge — deliberately NOT `upsert_internal`'s meta path
// (which REPLACES `answers` wholesale). See `extension_bridge::answers_save`
// for the caller.

#[test]
fn merge_answers_adds_new_answers_and_returns_count() {
    let (_dir, store) = open_store();
    let id = saved(&store, "https://acme.com/job/merge/1");

    let added = store
        .merge_answers(
            &id,
            vec![
                ans("Why this role?", "Because I love it."),
                ans("Salary expectation?", "100k"),
            ],
        )
        .unwrap();
    assert_eq!(added, 2);

    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 2);
    // Each merged answer gets a fresh, non-empty generated id.
    assert!(app.answers.iter().all(|a| !a.id.is_empty()));
}

#[test]
fn merge_answers_dedups_by_normalized_question_and_never_overwrites() {
    let (_dir, store) = open_store();
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ans_with_id("seed-1", "Why this role?", "Original")];
    let id = upsert(
        &store,
        "https://acme.com/job/merge/2",
        "linkedin",
        &m,
        ApplicationOrigin::Saved,
    );

    // A re-capture with different whitespace/case for the SAME question, plus
    // one genuinely new question.
    let added = store
        .merge_answers(
            &id,
            vec![
                ans("  WHY this   role?", "A newer answer that must be dropped"),
                ans("New question?", "New answer"),
            ],
        )
        .unwrap();
    assert_eq!(added, 1, "only the genuinely new question is added");

    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 2);
    let original = app
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .unwrap();
    assert_eq!(
        original.answer, "Original",
        "existing answer never overwritten"
    );
    assert_eq!(
        original.id, "seed-1",
        "existing answer's id is untouched too"
    );
}

#[test]
fn merge_answers_dedups_within_the_same_incoming_batch() {
    let (_dir, store) = open_store();
    let id = saved(&store, "https://acme.com/job/merge/3");

    // Two entries in ONE call that normalize to the same question — only the
    // first should be added.
    let added = store
        .merge_answers(
            &id,
            vec![
                ans("Why this role?", "First"),
                ans("why THIS role?", "Second (dropped)"),
            ],
        )
        .unwrap();
    assert_eq!(added, 1);
    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 1);
    assert_eq!(app.answers[0].answer, "First");
}

#[test]
fn merge_answers_leaves_updated_at_unchanged_when_nothing_new_added() {
    let (_dir, store) = open_store();
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ans_with_id("seed-1", "Why this role?", "Original")];
    let id = upsert(
        &store,
        "https://acme.com/job/merge/4",
        "linkedin",
        &m,
        ApplicationOrigin::Saved,
    );
    let before = store.get(&id).unwrap().updated_at;

    let added = store
        .merge_answers(&id, vec![ans("Why this role?", "Ignored")])
        .unwrap();
    assert_eq!(added, 0);
    assert_eq!(
        store.get(&id).unwrap().updated_at,
        before,
        "an all-dedup merge (nothing added) must not touch updated_at"
    );
}

#[test]
fn merge_answers_returns_error_for_unknown_id() {
    let (_dir, store) = open_store();
    let err = store.merge_answers("does-not-exist", vec![]).unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn merge_answers_caps_total_stored_answers_and_drops_the_rest() {
    let (_dir, store) = open_store();
    let mut m = meta("Acme", "Engineer");
    // Seed to exactly (cap - 2) existing distinct answers.
    m.answers = (0..MAX_TOTAL_ANSWERS - 2)
        .map(|i| {
            ans_with_id(
                format!("seed-{i}"),
                format!("Existing question {i}?"),
                format!("Existing answer {i}"),
            )
        })
        .collect();
    let id = upsert(
        &store,
        "https://acme.com/job/merge/cap",
        "linkedin",
        &m,
        ApplicationOrigin::Saved,
    );

    // 5 new distinct questions incoming — only 2 fit under the cap; the rest
    // are dropped (the caller derives `skipped` from `incoming_len - added`).
    let incoming: Vec<ApplicationAnswer> = (0..5)
        .map(|i| ans(format!("New question {i}?"), format!("New answer {i}")))
        .collect();

    let added = store.merge_answers(&id, incoming).unwrap();
    assert_eq!(added, 2, "only enough to reach the cap are added");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS,
        "total stored answers never exceeds the per-application cap"
    );
    // A seeded answer (by content, not just count) must survive the cap
    // untouched — the cap drops INCOMING overflow, never existing rows.
    let seeded = app
        .answers
        .iter()
        .find(|a| a.id == "seed-0")
        .expect("a seeded answer must survive the cap by id");
    assert_eq!(seeded.question, "Existing question 0?");
    assert_eq!(seeded.answer, "Existing answer 0");
}

/// HIGH regression: when `existing.answers` already sits AT or OVER
/// `MAX_TOTAL_ANSWERS` (a legacy row seeded before the on-creation cap
/// shipped), `merge_answers_by_question` used to remove the matching existing
/// answer from `merged` to make room for the incoming replacement, then the
/// cap check unconditionally blocked that same replacement from ever being
/// pushed back in — the question vanished entirely instead of being
/// rewritten. Seed a row with `MAX_TOTAL_ANSWERS + 1` answers via a raw SQL
/// UPDATE (bypassing the store's own cap enforcement, simulating a legacy
/// row), then upsert one incoming answer matching an existing question: it
/// must survive with the incoming text, and the total must not grow.
#[test]
fn merge_answers_by_question_swaps_a_replacement_even_when_existing_is_already_over_cap() {
    let (dir, store) = open_store();
    let url = "https://acme.com/job/merge/over-cap-swap";
    let id = saved(&store, url);

    // Seed MAX_TOTAL_ANSWERS + 1 distinct answers directly via raw SQL — a
    // legacy shape the on-creation cap (which runs on every upsert today)
    // would never itself produce, but a pre-existing over-cap row must still
    // be handled safely.
    let seeded: Vec<ApplicationAnswer> = (0..MAX_TOTAL_ANSWERS + 1)
        .map(|i| {
            ans_with_id(
                format!("seed-{i}"),
                format!("Existing question {i}?"),
                format!("Existing answer {i}"),
            )
        })
        .collect();
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET answers = ?1 WHERE id = ?2",
            params![serde_json::to_string(&seeded).unwrap(), id],
        )
        .unwrap();
    }

    // Re-upsert with one incoming answer matching an existing question (a
    // rewrite) — no genuinely new question, so this exercises the swap path
    // alone.
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ans("Existing question 0?", "Rewritten answer")];
    let id2 = upsert(&store, url, "linkedin", &m, ApplicationOrigin::Saved);
    assert_eq!(id, id2, "same url merges into the same Application");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS + 1,
        "a same-question swap must not grow (or shrink) an already over-cap row"
    );
    let swapped = app
        .answers
        .iter()
        .find(|a| a.question == "Existing question 0?")
        .expect("the matching question must survive the swap, not vanish");
    assert_eq!(
        swapped.answer, "Rewritten answer",
        "the incoming replacement must win, not silently disappear"
    );
    assert_eq!(
        app.answers
            .iter()
            .filter(|a| a.question != "Existing question 0?")
            .count(),
        MAX_TOTAL_ANSWERS,
        "every other seeded answer must be untouched"
    );
}

/// MEDIUM fix: `upsert_internal`'s NEW-ROW branch used to store `meta.answers`
/// verbatim, bypassing `MAX_TOTAL_ANSWERS` entirely (only the existing-row merge
/// branch enforced it). Creating a brand-new Application (no prior row for the
/// url) with an oversized, duplicate-question `meta.answers` must still come out
/// deduped-by-question and capped at `MAX_TOTAL_ANSWERS`.
#[test]
fn upsert_for_origin_caps_and_dedupes_answers_on_new_row_creation() {
    let (_dir, store) = open_store();

    // MAX_TOTAL_ANSWERS + 2 distinct questions, plus one duplicate (different
    // case/whitespace of question 0) inserted right after it — still over the
    // cap even after the duplicate is dropped.
    let mut answers: Vec<ApplicationAnswer> = (0..MAX_TOTAL_ANSWERS + 2)
        .map(|i| ans(format!("Question {i}?"), format!("Answer {i}")))
        .collect();
    answers.insert(1, ans("  question 0?  ", "Duplicate (dropped)"));

    let mut m = meta("Acme", "Engineer");
    m.answers = answers;

    let id = upsert(
        &store,
        "https://acme.com/job/new-row-cap",
        "linkedin",
        &m,
        ApplicationOrigin::Saved,
    );

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS,
        "a brand-new Application's answers are capped on creation, not just on merge"
    );
    // The genuine duplicate must never reach the stored row at all (dropped as a
    // within-batch dupe, not merely truncated by the cap).
    assert_eq!(
        app.answers
            .iter()
            .filter(|a| a.answer == "Duplicate (dropped)")
            .count(),
        0,
        "a duplicate-question answer must be deduped, not stored twice"
    );
    let first = app
        .answers
        .iter()
        .find(|a| a.question == "Question 0?")
        .expect("the first-seen answer for the duplicated question must survive");
    assert_eq!(first.answer, "Answer 0");
}

/// Regression for the HIGH cross-feature data-loss hazard: `upsert_internal`'s
/// meta-merge path used to REPLACE `answers` wholesale, so an
/// `ai_generations_save`-shaped upsert (a non-empty `meta.answers`) silently
/// wiped every answer the extension's `answers.save` had appended in
/// between. Seed extension-captured answers via `merge_answers`, then run an
/// `upsert_for_origin` carrying a DIFFERENT non-empty answer set (one
/// matching an existing question, one genuinely new) and assert: the
/// extension-only answer survives, the matching question's text is updated
/// to the incoming (AI) text, and the new AI answer is added.
#[test]
fn upsert_for_origin_merges_answers_by_question_instead_of_replacing() {
    let (_dir, store) = open_store();
    let url = "https://acme.com/job/merge/cross-feature";
    let id = saved(&store, url);

    // Extension-captured answers land first, via the append-only path.
    store
        .merge_answers(
            &id,
            vec![
                ans("Why this role?", "Extension-captured original"),
                ans(
                    "Are you willing to relocate?",
                    "Extension-only, no AI equivalent",
                ),
            ],
        )
        .unwrap();

    // Simulates `ai_generations_save`: a full generated answer set, one
    // question matching an existing extension answer (an in-app rewrite),
    // one genuinely new.
    let mut ai_meta = meta("Acme", "Engineer");
    ai_meta.answers = vec![
        ans("Why this role?", "AI-rewritten answer"),
        ans("What's your expected salary?", "100k"),
    ];
    upsert(
        &store,
        url,
        "linkedin",
        &ai_meta,
        ApplicationOrigin::Generate,
    );

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        3,
        "all 3 distinct questions must be present"
    );

    let relocate = app
        .answers
        .iter()
        .find(|a| a.question == "Are you willing to relocate?")
        .expect("the extension-only answer must survive the AI upsert");
    assert_eq!(relocate.answer, "Extension-only, no AI equivalent");

    let rewritten = app
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .expect("the matching question must still be present");
    assert_eq!(
        rewritten.answer, "AI-rewritten answer",
        "a matching question must be updated to the incoming text"
    );

    assert!(
        app.answers
            .iter()
            .any(|a| a.question == "What's your expected salary?" && a.answer == "100k"),
        "a genuinely new AI answer must be added"
    );
}
