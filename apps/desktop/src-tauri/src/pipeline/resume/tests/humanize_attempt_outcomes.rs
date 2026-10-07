use super::super::prompts::HumanizeTier;
use super::super::stages::{humanize_one, voice_count};
use super::support::{criticals, live_deadline, ok_report, voice_report};

/// **HIGH-2 regression, letter half — end to end through the seam, not just
/// the pure predicate.** A letter candidate truncated to ~60% of the original
/// is unusable at the letter's strict 90% floor: the answer is kept as
/// unusable, exactly like `humanize_one_keeps_the_original_when_the_answer_is_unusable`
/// above, and — critically — `revalidate` never runs over it (a 40%-shorter
/// letter has no absence-shaped Critical to catch that loss; the floor IS the
/// backstop, so this must never reach the point where a validator could wave
/// it through).
#[tokio::test]
async fn humanize_one_keeps_the_original_letter_when_the_candidate_is_truncated_to_sixty_percent() {
    let original = "A".repeat(100);
    let candidate_60_percent = "B".repeat(60);
    let attempt = humanize_one(
        live_deadline(),
        original.clone(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| {
            let candidate = candidate_60_percent.clone();
            async move { Ok(candidate) }
        },
        |_candidate: &str| None,
        |_candidate| async move { panic!("revalidate must never run over an unusable letter") },
        HumanizeTier::Letter,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(attempt.called);
    assert!(
        !attempt.failed,
        "the call succeeded — the ANSWER was unusable"
    );
    assert!(
        !attempt.reverted,
        "reverted implies it was graded; this never was"
    );
    assert_eq!(attempt.text, original);
}

/// **HIGH-2 regression, résumé half — the SAME 60% candidate the letter test
/// above rejects is accepted here**, because the résumé tier's 50% floor is
/// generous and its real backstop against content loss is
/// `humanize_is_worse`'s absence-shaped Criticals, not the length check. A
/// clean revalidate (no new Criticals, no more voice flags) is enough to ship
/// it — this is what pins the two tiers as genuinely different floors rather
/// than one shared constant that happens to read differently.
#[tokio::test]
async fn humanize_one_accepts_a_resume_candidate_truncated_to_sixty_percent_when_validators_are_clean(
) {
    let original = "A".repeat(100);
    let candidate_60_percent = "B".repeat(60);
    let attempt = humanize_one(
        live_deadline(),
        original,
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| {
            let candidate = candidate_60_percent.clone();
            async move { Ok(candidate) }
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("revalidate succeeds");
    assert!(!attempt.reverted);
    assert_eq!(attempt.text, "B".repeat(60));
}

/// **Revert on an introduced Critical** — `humanize_is_worse`'s `round_is_worse`
/// half, exercised end to end through the seam.
#[tokio::test]
async fn humanize_one_reverts_when_the_candidate_introduces_a_critical() {
    let before_report = voice_report(&["robust"]);
    let attempt =
        humanize_one(
            live_deadline(),
            "ORIGINAL TEXT THAT IS REASONABLY LONG".to_string(),
            before_report,
            vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
            |_text, _findings| async move {
                Ok("REWRITTEN TEXT THAT IS ALSO REASONABLY LONG".to_string())
            },
            |_candidate: &str| None,
            |_candidate| async move {
                let mut after = criticals(1);
                after.issues[0].evidence = Some("an invented figure".to_string());
                Ok(after)
            },
            HumanizeTier::Resume,
            true,
        )
        .await
        .expect("revalidate succeeds");
    assert!(attempt.called);
    assert!(attempt.reverted);
    assert!(!attempt.failed);
    assert_eq!(attempt.text, "ORIGINAL TEXT THAT IS REASONABLY LONG");
}

/// **Revert on MORE voice flags, with zero new Criticals** — the clause
/// `round_is_worse` alone cannot express.
#[tokio::test]
async fn humanize_one_reverts_when_the_candidate_has_more_voice_flags_than_before() {
    let before_report = voice_report(&["robust"]);
    let attempt =
        humanize_one(
            live_deadline(),
            "ORIGINAL TEXT THAT IS REASONABLY LONG".to_string(),
            before_report,
            vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
            |_text, _findings| async move {
                Ok("REWRITTEN TEXT THAT IS ALSO REASONABLY LONG".to_string())
            },
            |_candidate: &str| None,
            |_candidate| async move { Ok(voice_report(&["robust", "leverage"])) },
            HumanizeTier::Resume,
            true,
        )
        .await
        .expect("revalidate succeeds");
    assert!(attempt.reverted);
    assert_eq!(attempt.text, "ORIGINAL TEXT THAT IS REASONABLY LONG");
}

/// **Accept a candidate that strictly improves — fewer voice flags, no new
/// Criticals.** The candidate and its FRESH report both ship.
#[tokio::test]
async fn humanize_one_accepts_a_candidate_with_fewer_voice_flags() {
    let before_report = voice_report(&["robust", "leverage"]);
    let attempt =
        humanize_one(
            live_deadline(),
            "ORIGINAL TEXT THAT IS REASONABLY LONG".to_string(),
            before_report,
            vec![
                "[voice.ai_tell_lexical] robust".to_string(),
                "[voice.ai_tell_lexical] leverage".to_string(),
            ],
            |_text, _findings| async move {
                Ok("REWRITTEN TEXT THAT IS ALSO REASONABLY LONG".to_string())
            },
            |_candidate: &str| None,
            |_candidate| async move { Ok(voice_report(&["robust"])) },
            HumanizeTier::Resume,
            true,
        )
        .await
        .expect("revalidate succeeds");
    assert!(!attempt.reverted);
    assert_eq!(attempt.text, "REWRITTEN TEXT THAT IS ALSO REASONABLY LONG");
    assert_eq!(voice_count(&attempt.report), 1);
}

/// **The normalize pass runs BETWEEN the provider's answer and revalidation,
/// and its output — not the model's raw text — is what gets graded and kept.**
/// This is what makes an accepted résumé rewrite link-safe BY CONSTRUCTION: a
/// project link line the model altered is restored before the document is
/// ever judged, so the accepted text is byte-identical to the source's link
/// line rather than merely "not flagged this time".
///
/// Mutation check: apply `normalize` AFTER `revalidate` instead of before it,
/// and the `revalidate` closure's own assertion (it must see the NORMALIZED
/// text) fails.
#[tokio::test]
async fn humanize_one_runs_normalize_before_grading_so_an_accepted_candidate_is_link_safe() {
    let original = "PROJECTS\nLedger CLI — https://github.com/jane/ledger\n";
    let tampered = "PROJECTS\nLedger CLI — https://evil.example/ledger\n"; // what the model returned
    let restored = original.to_string(); // what `normalize` restores it to
    let normalize_called = std::cell::Cell::new(false);
    let attempt = humanize_one(
        live_deadline(),
        original.to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| {
            let tampered = tampered.to_string();
            async move { Ok(tampered) }
        },
        |candidate: &str| {
            normalize_called.set(true);
            assert_eq!(candidate, tampered, "normalize sees the model's RAW answer");
            Some(restored.clone())
        },
        |candidate| {
            let restored_check = restored.clone();
            async move {
                assert_eq!(
                    candidate, restored_check,
                    "revalidate must see the NORMALIZED candidate, never the model's raw answer"
                );
                Ok(ok_report())
            }
        },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("revalidate succeeds");
    assert!(normalize_called.get());
    assert!(!attempt.reverted);
    assert_eq!(
        attempt.text, original,
        "the accepted text is byte-identical to the source's own link line"
    );
}
