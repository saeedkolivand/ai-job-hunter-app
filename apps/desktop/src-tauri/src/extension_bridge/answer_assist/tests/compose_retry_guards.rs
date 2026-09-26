//! `compose_with_length_retry` — spend/guard discipline: when NOT to retry,
//! the daily ceiling gating the retry, no terminal frame for a never-run
//! stream, and the effort tier riding through both attempts unchanged.

use crate::error::AppError;

use super::super::budgets::{ANSWER_ASSIST_MAX_TOKENS, ANSWER_ASSIST_RETRY_MAX_TOKENS};
use super::super::compose::compose_with_length_retry;
use super::super::errors::DRAFT_FAILED_MESSAGE;
use super::super::AssistMode;
use super::support::{
    done_frames, empty_answer, fails, length_cut, streams, FakeComposer, RecordingSink,
};
use crate::extension_bridge::answer_assist_parse::assist_prompt_for_mode;

/// A cancel (or a dropped connection) between the two attempts takes this
/// request's registry entry away — the retry must then never be charged for,
/// let alone composed.
///
/// Mutation check (executed): drop the `still_wanted` guard from
/// `compose_attempts` and both assertions fail.
#[tokio::test]
async fn compose_with_length_retry_refuses_to_pay_for_a_retry_the_client_gave_up_on() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![fails(length_cut()), streams("never reached")],
        &mut sink,
    );
    round.wanted = false; // an assist.cancel / disconnect landed in between

    let err = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect_err("an abandoned request still fails");

    assert_eq!(err.to_string(), DRAFT_FAILED_MESSAGE);
    assert_eq!(
        round.charges.get(),
        1,
        "the second round-trip must never be charged for"
    );
    assert_eq!(round.attempts.len(), 1, "…nor composed");
    assert_eq!(
        round.finishes, 1,
        "the request still owes its one terminal frame — a stream did run"
    );
}

#[tokio::test]
async fn compose_with_length_retry_charges_and_composes_once_when_the_first_attempt_succeeds() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(vec![streams("First time lucky.")], &mut sink);

    let text = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect("the first attempt succeeds");

    assert_eq!(text, "First time lucky.");
    assert_eq!(
        round.attempts,
        vec![(ANSWER_ASSIST_MAX_TOKENS, Some("low".to_string()))]
    );
    assert_eq!(
        round.charges.get(),
        1,
        "one round-trip, one daily-ceiling charge"
    );
    assert_eq!(done_frames(&sink.sent), 1, "one terminal frame, as always");
}

#[tokio::test]
async fn compose_with_length_retry_never_retries_any_other_failure() {
    // Each of these is a DIFFERENT way the compose can fail: a transport
    // error; the GENERIC empty answer (same empty outcome, but no
    // `finish_reason: length`, so nothing says a larger budget would help);
    // and the length-cut TEXT carried by a variant `finish` never builds it
    // as — classification is structural, not a substring search. None of
    // them may buy a second billable round-trip.
    let others = [
        AppError::Network("connection reset".to_string()),
        empty_answer(None),
        AppError::Validation(length_cut().to_string()),
    ];

    for original in others {
        let label = original.to_string();
        let mut sink = RecordingSink::default();
        let mut round =
            FakeComposer::new(vec![fails(original), streams("never reached")], &mut sink);

        let err = compose_with_length_retry(
            &mut round,
            ANSWER_ASSIST_MAX_TOKENS,
            ANSWER_ASSIST_RETRY_MAX_TOKENS,
            Some("low"),
        )
        .await
        .expect_err("a non-length-cut failure must surface, not retry");

        assert_eq!(
            err.to_string(),
            DRAFT_FAILED_MESSAGE,
            "every failure still collapses to the fixed wire sentinel"
        );
        assert_eq!(round.attempts.len(), 1, "{label} must NOT be retried");
        assert_eq!(
            round.charges.get(),
            1,
            "{label} must cost exactly one charge"
        );
        assert_eq!(
            done_frames(&sink.sent),
            1,
            "{label} still owes its one terminal frame"
        );
    }
}

#[tokio::test]
async fn compose_with_length_retry_lets_the_daily_ceiling_refuse_the_retry() {
    // The retry is real spend: it goes through the SAME charge the first
    // attempt does, so a ceiling that refuses it stops the second
    // round-trip from ever being made.
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(
        vec![fails(length_cut()), streams("never reached")],
        &mut sink,
    );
    round.refuse_charge_at = Some(2);

    let err = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect_err("a refused charge fails the request");

    assert_eq!(err.to_string(), DRAFT_FAILED_MESSAGE);
    assert_eq!(
        round.attempts.len(),
        1,
        "the retry must never bypass the daily ceiling"
    );
}

/// The FIRST charge sits outside the attempt block on purpose: when the daily
/// ceiling refuses it, no stream ever ran, so the request owes its client no
/// terminal frame at all — only the `answer.assist.result` error reply. This
/// is the one path `finish` must NOT run on.
#[tokio::test]
async fn compose_with_length_retry_emits_no_terminal_frame_when_the_first_charge_is_refused() {
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(vec![streams("never reached")], &mut sink);
    round.refuse_charge_at = Some(1);

    let err = compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        Some("low"),
    )
    .await
    .expect_err("a refused charge fails the request");

    assert_eq!(err.to_string(), DRAFT_FAILED_MESSAGE);
    assert!(round.attempts.is_empty(), "no round-trip was ever made");
    assert_eq!(round.finishes, 0);
    assert!(
        sink.sent.is_empty(),
        "…so nothing was framed for the client"
    );
}

#[tokio::test]
async fn compose_with_length_retry_sends_no_effort_for_a_model_with_no_cheap_tier() {
    // `Completer::low_effort` resolves `None` both for a model whose
    // provider offers no effort levels at all and for one whose lowest tier
    // is already expensive (see `pipeline::low_effort_level`). That `None`
    // must reach the request unchanged on BOTH attempts — never a
    // substituted "low" the provider would reject.
    let mut sink = RecordingSink::default();
    let mut round = FakeComposer::new(vec![fails(length_cut()), streams("answer")], &mut sink);

    compose_with_length_retry(
        &mut round,
        ANSWER_ASSIST_MAX_TOKENS,
        ANSWER_ASSIST_RETRY_MAX_TOKENS,
        None,
    )
    .await
    .expect("the retry succeeds");

    assert_eq!(
        round.attempts,
        vec![
            (ANSWER_ASSIST_MAX_TOKENS, None),
            (ANSWER_ASSIST_RETRY_MAX_TOKENS, None),
        ]
    );
}

/// The two budget constants' own numeric relationships are asserted at COMPILE
/// time next to them (`answer_assist/budgets.rs`'s `const _: () = { … }`) — a
/// build failure beats a test failure for a pair of constants. What still
/// needs a test is the MODE TABLE reading the same one for both modes.
#[test]
fn both_modes_compose_at_the_same_first_attempt_budget() {
    assert_eq!(
        assist_prompt_for_mode(AssistMode::Draft).1,
        assist_prompt_for_mode(AssistMode::Rewrite).1,
        "draft and rewrite share the budget deliberately — see `assist_prompt_for_mode`"
    );
    assert_eq!(
        assist_prompt_for_mode(AssistMode::Draft).1,
        ANSWER_ASSIST_MAX_TOKENS
    );
}
