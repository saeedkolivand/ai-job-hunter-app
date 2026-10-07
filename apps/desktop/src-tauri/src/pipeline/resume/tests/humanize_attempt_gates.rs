use super::super::prompts::{HumanizeTier, HUMANIZE_DOCUMENT_CAP};
use super::super::stages::humanize_one;
use super::support::{expired_deadline, live_deadline, ok_report, voice_report};

/// **Zero findings is a zero-cost no-op** — the gate the stage itself applies
/// before ever building a system prompt, pinned again here at the seam that
/// actually enforces it.
///
/// Mutation check: drop the `findings.is_empty()` early return and the
/// `called` flag flips true.
#[tokio::test]
async fn humanize_one_is_a_zero_cost_no_op_with_no_findings() {
    let mut called = false;
    let attempt = humanize_one(
        live_deadline(),
        "ORIGINAL".to_string(),
        ok_report(),
        Vec::<String>::new(),
        |_text, _findings| {
            called = true;
            async move { Ok("must never run".to_string()) }
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(!called);
    assert!(!attempt.called);
    assert!(!attempt.reverted && !attempt.failed && !attempt.timed_out);
    assert_eq!(attempt.text, "ORIGINAL");
}

/// **An over-cap document is refused BEFORE the deadline or findings are even
/// looked at** — `fenced()` would silently truncate it, and a faithful
/// rewrite of a truncated prefix can still clear the résumé's length floor
/// (see `exceeds_humanize_cap`'s own doc). Zero calls, the original kept
/// byte-for-byte, and `too_large` recorded instead of a fabricated success.
///
/// Mutation check: drop the `exceeds_humanize_cap` gate from `humanize_one`
/// and `called` flips true.
#[tokio::test]
async fn humanize_one_refuses_a_document_over_the_cap_without_a_single_call() {
    let mut called = false;
    let over_cap = "A".repeat(HUMANIZE_DOCUMENT_CAP + 1);
    let attempt = humanize_one(
        live_deadline(),
        over_cap.clone(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| {
            called = true;
            async move { Ok("must never run".to_string()) }
        },
        |_candidate: &str| None,
        |_candidate| async move { panic!("revalidate must never run over the cap") },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(
        !called,
        "no provider call once the document is over the cap"
    );
    assert!(!attempt.called);
    assert!(attempt.too_large);
    assert!(!attempt.reverted && !attempt.failed && !attempt.timed_out);
    assert_eq!(
        attempt.text, over_cap,
        "the original document is kept, untruncated"
    );
}

/// **The run's deadline is checked right after the size cap, before findings
/// are even looked at** — the in-stage check `repair.rs`'s own module doc
/// argues for, applied here because `humanize` is now the LAST stage.
///
/// Mutation check: check `findings.is_empty()` before `deadline.passed()` and
/// this still passes (findings is non-empty here) but the NEXT test — an
/// expired deadline with findings present — is what actually catches a
/// reordering; both are asserted for the same reason a single case would not
/// separate the two gates.
#[tokio::test]
async fn humanize_one_skips_gracefully_when_the_deadline_has_already_passed() {
    let mut called = false;
    let attempt = humanize_one(
        expired_deadline(),
        "ORIGINAL".to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| {
            called = true;
            async move { Ok("must never run".to_string()) }
        },
        |_candidate: &str| None,
        |_candidate| async move { Ok(ok_report()) },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(!called, "no provider call once the deadline has passed");
    assert!(!attempt.called);
    assert!(attempt.timed_out);
    assert!(!attempt.reverted && !attempt.failed);
    assert_eq!(
        attempt.text, "ORIGINAL",
        "the original document is kept, not discarded"
    );
}

/// **A provider error never fails the run — it keeps the original and marks
/// `failed`.** Mirrors `repair_loop`'s own per-section error policy.
#[tokio::test]
async fn humanize_one_keeps_the_original_and_marks_failed_on_a_provider_error() {
    let attempt = humanize_one(
        live_deadline(),
        "ORIGINAL".to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| async move {
            Err(crate::error::AppError::Provider(
                "the provider is unreachable".to_string(),
            ))
        },
        |_candidate: &str| None,
        |_candidate| async move { panic!("revalidate must never run after a provider error") },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("a provider error is caught inside humanize_one, never propagated");
    assert!(attempt.called, "a call WAS attempted");
    assert!(attempt.failed);
    assert!(!attempt.reverted && !attempt.timed_out);
    assert_eq!(attempt.text, "ORIGINAL");
}

/// **A revalidate error never fails the WHOLE RUN — it keeps the original and
/// marks `failed`**, exactly like a provider error above. Before this fix,
/// the `?` on `revalidate(...)` was the only path through `humanize_one` that
/// could propagate a failure out of what is, by design, this stage's own
/// best-effort cleanup pass (a `spawn_blocking` join failure inside
/// `validate_documents` is the process, not the model).
///
/// Mutation check: restore the `?` on the revalidate call and this test's
/// `.expect(...)` on the outer `Result` panics instead of asserting.
#[tokio::test]
async fn humanize_one_keeps_the_original_and_marks_failed_when_revalidate_errors() {
    let attempt = humanize_one(
        live_deadline(),
        "ORIGINAL".to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| async move { Ok("REWRITTEN".to_string()) },
        |_candidate: &str| None,
        |_candidate| async move {
            Err(crate::error::AppError::Provider(
                "revalidate could not be joined".to_string(),
            ))
        },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("a revalidate error is caught inside humanize_one, never propagated");
    assert!(attempt.called, "a call WAS attempted");
    assert!(attempt.failed);
    assert!(!attempt.reverted && !attempt.timed_out);
    assert_eq!(
        attempt.text, "ORIGINAL",
        "the original is kept, not the ungraded candidate"
    );
}

/// An empty/truncated answer is graded as UNUSABLE before it ever reaches
/// revalidation — never a "successful" call that happened to fail the
/// accept/revert comparison.
#[tokio::test]
async fn humanize_one_keeps_the_original_when_the_answer_is_unusable() {
    let original = "PROFESSIONAL SUMMARY\nA payments engineer with ten years of experience \
                     building ledger systems.\n";
    let attempt = humanize_one(
        live_deadline(),
        original.to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| async move { Ok(String::new()) },
        |_candidate: &str| None,
        |_candidate| async move { panic!("revalidate must never run over an unusable answer") },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(attempt.called);
    assert!(
        !attempt.failed,
        "the call succeeded — the ANSWER was unusable, not the transport"
    );
    assert!(
        !attempt.reverted,
        "reverted implies it was graded; this never was"
    );
    assert_eq!(attempt.text, original);
}

/// **BUG-A regression, end to end through the seam.** A candidate that echoes
/// the SAME `<humanize_document>` tag the real incident leaked is treated
/// exactly like a truncated/empty answer: discarded before revalidation ever
/// runs (the `panic!` below would fire if it did), `called` recorded honestly,
/// `reverted` false because nothing was graded — the shape gate rejected it
/// first — and the original text kept byte-for-byte.
#[tokio::test]
async fn humanize_one_keeps_the_original_when_the_answer_echoes_its_own_fence_tag() {
    let original =
        "PROFESSIONAL SUMMARY\nA payments engineer with ten years of experience building \
         ledger systems.\n";
    let attempt = humanize_one(
        live_deadline(),
        original.to_string(),
        voice_report(&["robust"]),
        vec!["[voice.ai_tell_lexical] on the ban list".to_string()],
        |_text, _findings| async move {
            Ok(
                "<humanize_document>\nPROFESSIONAL SUMMARY\nA payments engineer with a decade \
                of experience building ledger systems.\n</humanize_document>"
                    .to_string(),
            )
        },
        |_candidate: &str| None,
        |_candidate| async move { panic!("revalidate must never run over a shape-broken answer") },
        HumanizeTier::Resume,
        true,
    )
    .await
    .expect("no revalidate call means no error path either");
    assert!(attempt.called);
    assert!(
        !attempt.reverted,
        "reverted implies it was graded; the shape gate rejected it first"
    );
    assert_eq!(attempt.text, original);
}
