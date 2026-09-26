//! `compose_with_length_retry` — the retry mechanics: one retry at the larger
//! budget, exactly one terminal frame per REQUEST, each attempt capped at its
//! own `DRAFT_CAP` window, and the draft returned is the SUCCESSFUL attempt's
//! text alone.

use super::super::budgets::{ANSWER_ASSIST_MAX_TOKENS, ANSWER_ASSIST_RETRY_MAX_TOKENS, DRAFT_CAP};
use super::super::compose::{compose_with_length_retry, DraftComposer};
use super::support::{
    done_frames, fails, forwarded_chars, length_cut, streams, streams_then_fails, FakeComposer,
    RecordingSink,
};

#[tokio::test]
async fn compose_with_length_retry_retries_once_at_the_retry_budget_after_an_empty_length_cut() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![fails(length_cut()), streams("A grounded answer.")],
        &mut sink,
    );

    let text = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(text, "A grounded answer.");
    assert_eq!(
        round.attempts,
        vec![
            (ANSWER_ASSIST_MAX_TOKENS, Some("low".to_string())),
            (ANSWER_ASSIST_RETRY_MAX_TOKENS, Some("low".to_string())),
        ],
        "the retry must run at the LARGER budget, at the same cheap effort"
    );
    assert_eq!(
        round.charges.get(),
        2,
        "two round-trips must pay the daily ceiling twice — never once per request"
    );
    assert!(
        sink.sent[0].contains("A grounded answer."),
        "the retry's text must reach the sink, got {:?}",
        sink.sent
    );
}

/// The `assist.done` contract is per REQUEST, not per attempt (see
/// `compose_with_length_retry`'s doc).
///
/// Mutation check (executed): move `round.finish()` inside `compose_attempts`
/// so it runs per attempt (the pre-fix shape) — this test fails on the count
/// AND on the ordering assertion.
#[tokio::test]
async fn compose_with_length_retry_sends_exactly_one_assist_done_after_every_chunk() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![fails(length_cut()), streams("the retry's own text")],
        &mut sink,
    );

    compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(round.finishes, 1, "one terminal frame per REQUEST");
    assert_eq!(
        done_frames(&sink.sent),
        1,
        "…and exactly one reaches the wire, got {:?}",
        sink.sent
    );
    let last = sink.sent.last().expect("frames were sent");
    assert_eq!(
        done_frames(std::slice::from_ref(last)),
        1,
        "the terminal frame must be LAST — a chunk after it is a chunk the popup drops"
    );
    assert!(
        sink.sent[0].contains("the retry's own text"),
        "the retry's chunks must reach the sink BEFORE the terminal frame, got {:?}",
        sink.sent
    );
}

/// `DRAFT_CAP` bounds each ATTEMPT, not the request: the retry gets a full
/// window of its own, rebased at its own start. Attempt 1 here both forwards
/// text AND ends as an empty length cut (see `FakeAttempt`) and spends all
/// but 10 chars of a cap doing it. Against one shared window the retry's
/// 100-char answer would come back as a 10-char stub with `ok: true`: a
/// silently truncated draft, straight into the field "Accept" pastes. See
/// [`compose_with_length_retry_still_clamps_the_retry_at_one_draft_cap`] for
/// the other half of that bound.
///
/// Mutation check (executed): pass `0` as the retry's `cap_base` (the pre-fix
/// shared window) and both assertions fail — the draft is 10 chars and the
/// wire total is exactly `DRAFT_CAP`.
#[tokio::test]
async fn compose_with_length_retry_gives_the_retry_a_cap_the_failed_attempt_did_not_spend() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![
            streams_then_fails("x".repeat(DRAFT_CAP - 10), length_cut()),
            streams("y".repeat(100)),
        ],
        &mut sink,
    );

    let text = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(
        text,
        "y".repeat(100),
        "the retry's answer must arrive whole — a failed attempt's spend may \
         not truncate it"
    );
    assert_eq!(
        forwarded_chars(&sink.sent),
        DRAFT_CAP - 10 + 100,
        "the wire carries attempt 1's prose plus the retry's whole answer"
    );
}

/// The other half of the bound: one attempt still never forwards more than
/// `DRAFT_CAP` chars, so a retried request is bounded at 2 × `DRAFT_CAP` and
/// the draft returned is bounded at `DRAFT_CAP` — a rebased window is a FRESH
/// budget, never an unbounded one.
///
/// Mutation check (executed): let a rebased window pass its delta through
/// unclamped (guard `forward_chunk`'s clamp on `cap_base == 0`) and both
/// assertions fail — the retry forwards, and returns, `DRAFT_CAP + 50` chars.
#[tokio::test]
async fn compose_with_length_retry_still_clamps_the_retry_at_one_draft_cap() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![
            streams_then_fails("x".repeat(DRAFT_CAP - 10), length_cut()),
            streams("y".repeat(DRAFT_CAP + 50)),
        ],
        &mut sink,
    );

    let text = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(
        text.chars().count(),
        DRAFT_CAP,
        "the retry's own window is one cap, not two"
    );
    assert_eq!(
        forwarded_chars(&sink.sent),
        DRAFT_CAP - 10 + DRAFT_CAP,
        "so the whole request stays under 2 x DRAFT_CAP on the wire"
    );
}

/// The buffer the two attempts share is a wire LOG, never the request's
/// result: the draft returned is the text of the attempt that SUCCEEDED,
/// alone (see `attempt_text`'s doc for the local-model shape that makes
/// attempt 1 forward visible prose and STILL end as the empty length cut).
///
/// Mutation check (executed): return the whole shared buffer from
/// `compose_attempts` (`Ok(round.drafted().to_string())`, the pre-fix shape)
/// and this test fails — the draft comes back with the reasoning prefix.
#[tokio::test]
async fn compose_with_length_retry_returns_only_the_successful_attempts_text() {
    // Multi-byte on purpose: the tail is cut at a BYTE offset of a buffer that is
    // only ever clamped by CHARS, so an ASCII fixture would not exercise the seam.
    const THOUGHT: &str = "<think>Réfléchissons — l'utilisateur veut une réponse courte 🤔";
    const ANSWER: &str = "Bonjour, ça va très bien 🙂";
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![streams_then_fails(THOUGHT, length_cut()), streams(ANSWER)],
        &mut sink,
    );

    let text = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(
        text, ANSWER,
        "the draft is the retry's answer alone — a failed attempt's forwarded \
         text must never ride back with it"
    );
    // …while the failed attempt's chars are still THERE: they went out on the
    // wire as `assist.chunk` frames and the buffer is append-only, which is
    // what makes the tail slice possible at all. The buffer spans both
    // attempts; what the client gets back does not.
    assert!(
        round.drafted().starts_with(THOUGHT) && round.drafted().ends_with(ANSWER),
        "the append-only buffer keeps BOTH attempts, got {:?}",
        round.drafted()
    );
    assert_eq!(
        round.drafted().chars().count(),
        THOUGHT.chars().count() + ANSWER.chars().count(),
        "and it is the sum of the two attempts, each bounded by its own DRAFT_CAP window"
    );
}
