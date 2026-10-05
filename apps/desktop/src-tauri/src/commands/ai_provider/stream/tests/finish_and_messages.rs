//! `empty_answer_message` / `is_empty_answer_length_cut` / `finish_outcome` /
//! `truncation_notification` tests.
//!
//! `finish` and `cli_agent::emit_done` both route their empty-answer `Err`
//! message through the pure `empty_answer_message` decision, so it is
//! directly testable without the `AppHandle` this crate has no test harness
//! for (see e.g. `openai/tests/capabilities.rs`'s note on the same limitation).
//!
//! `finish` used to consult `stop_reason` only on the EMPTY path — a stream
//! that emitted real text and then reported `finish_reason: "length"` took
//! the plain success path with no signal anywhere that the saved document
//! might be cut off. `finish_outcome` is the pure decision `finish` now
//! delegates to, directly testable the same way.

use super::super::finish::*;
use super::super::*;

#[test]
fn empty_answer_message_reports_the_length_truncation_distinctly() {
    assert_eq!(
        empty_answer_message(Some(StopReason::Length), ProviderId::OpenAi),
        EMPTY_ANSWER_LENGTH_MESSAGE,
        "finish_reason: length must get its own, more actionable message"
    );
}

/// Local Ollama is the ONE provider whose output cap the user can raise in
/// the app (`LocalModelLimits`, rendered only under the Ollama card), so it
/// gets the actionable wording. Every other provider must keep the generic
/// one — pointing them at a control they cannot see is worse than no advice,
/// which is why the original message dropped the pointer entirely.
///
/// This distinction only became reachable when local Ollama started
/// reporting `done_reason`; before that it never hit the `Length` arm.
#[test]
fn empty_answer_message_points_local_ollama_at_the_control_it_actually_has() {
    assert_eq!(
        empty_answer_message(Some(StopReason::Length), ProviderId::Ollama),
        EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE
    );
    assert!(
        EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE.contains("Max output tokens"),
        "must name the field as the UI labels it"
    );
    // The differential — no other provider may be sent to that field.
    for p in [
        ProviderId::OllamaCloud,
        ProviderId::OpenAi,
        ProviderId::OpenAiCompatible,
        ProviderId::Anthropic,
        ProviderId::Gemini,
    ] {
        assert_eq!(
            empty_answer_message(Some(StopReason::Length), p),
            EMPTY_ANSWER_LENGTH_MESSAGE,
            "{} has no adjustable output cap in the UI",
            p.as_str()
        );
    }
}

/// The classification the extension bridge's `answer.assist` retries on —
/// built from what `finish` ACTUALLY returns for an empty completion (its
/// `FinishOutcome::Empty` message, wrapped in the `AppError::Provider` its
/// own branch wraps it in), not from a re-typed message string. If `finish`
/// ever changed variant, or `empty_answer_message` gained an arm this
/// predicate does not know about, the retry would silently stop firing (or
/// start firing on the wrong failure) with nothing else to catch it.
#[test]
fn is_empty_answer_length_cut_recognizes_exactly_the_length_truncation_finish_returns() {
    let as_finish_returns_it =
        |stop_reason, provider| match finish_outcome("", stop_reason, provider) {
            FinishOutcome::Empty { message } => AppError::Provider(message.to_string()),
            FinishOutcome::Complete { .. } => panic!("an empty answer is never Complete"),
        };

    // BOTH length-cut wordings — the generic one and local Ollama's
    // Settings-pointing sibling — are the same failure to a caller.
    for provider in [
        ProviderId::OllamaCloud,
        ProviderId::Ollama,
        ProviderId::OpenAi,
    ] {
        assert!(
            is_empty_answer_length_cut(&as_finish_returns_it(Some(StopReason::Length), provider)),
            "{} ran out of budget mid-reasoning",
            provider.as_str()
        );
    }

    // An empty answer with no `length` signal is NOT this failure: nothing
    // says a larger budget would change the outcome, so it must not buy a
    // second billable round-trip.
    for reason in [None, Some(StopReason::End), Some(StopReason::Other)] {
        assert!(!is_empty_answer_length_cut(&as_finish_returns_it(
            reason,
            ProviderId::OllamaCloud
        )));
    }

    // Structural, not a substring search: the same text in a variant
    // `finish` never builds is a different failure.
    assert!(!is_empty_answer_length_cut(&AppError::Validation(
        EMPTY_ANSWER_LENGTH_MESSAGE.to_string()
    )));
    assert!(!is_empty_answer_length_cut(&AppError::Provider(
        "429 rate limited".to_string()
    )));
}

#[test]
fn empty_answer_message_falls_back_to_the_generic_message_otherwise() {
    // No signal at all (most providers/CLI agents) and a non-`Length`
    // signal both fall back to the SAME generic message — only `Length`
    // is distinct, per the report this closes.
    for reason in [None, Some(StopReason::End), Some(StopReason::Other)] {
        assert_eq!(
            empty_answer_message(reason, ProviderId::OpenAi),
            EMPTY_ANSWER_MESSAGE
        );
    }
}

/// The exact scenario from the report: real text streamed, then the
/// provider's `finish_reason` came back `length`. The completion must
/// still be treated as a SUCCESS (the partial text is real and worth
/// keeping) — never re-routed into the `Empty`/error arm — AND a warning
/// must be attached so the caller can surface it.
#[test]
fn a_truncated_non_empty_completion_still_completes_with_the_partial_text_and_warns() {
    let outcome = finish_outcome(
        "here is some partial output that got cut off",
        Some(StopReason::Length),
        ProviderId::OpenAi,
    );
    match outcome {
        FinishOutcome::Complete { text, warning } => {
            assert_eq!(text, "here is some partial output that got cut off");
            let warning = warning.expect("a non-empty Length completion must warn");
            assert_eq!(warning.kind, TRUNCATED_NOTIFICATION_KIND);
            assert!(
                warning.body.to_lowercase().contains("budget")
                    || warning.body.to_lowercase().contains("cut off"),
                "warning body must actually explain the truncation, got: {}",
                warning.body
            );
        }
        FinishOutcome::Empty { .. } => {
            panic!("a non-empty answer must never be routed through the Empty arm")
        }
    }
}

/// A normal completion (`stop_reason: End`, or none at all) must produce
/// NO warning — the whole point is that this is a targeted signal for the
/// truncation case, not noise on every generation.
#[test]
fn a_normal_completion_produces_no_warning() {
    for reason in [None, Some(StopReason::End), Some(StopReason::ToolUse)] {
        let outcome = finish_outcome("a complete answer", reason, ProviderId::OpenAi);
        match outcome {
            FinishOutcome::Complete { text, warning } => {
                assert_eq!(text, "a complete answer");
                assert!(
                    warning.is_none(),
                    "stop_reason {reason:?} must not produce a truncation warning"
                );
            }
            FinishOutcome::Empty { .. } => panic!("non-empty answer, must not be Empty"),
        }
    }
}

/// The EMPTY path is untouched by this change: `finish_reason: length` with
/// NO text at all still routes through the pre-existing empty-answer
/// message, never `truncation_notification` (that path already has its own
/// distinct, more actionable message — see `empty_answer_message`).
#[test]
fn an_empty_length_completion_still_uses_the_empty_answer_message_not_a_warning() {
    let outcome = finish_outcome("   ", Some(StopReason::Length), ProviderId::OpenAi);
    match outcome {
        FinishOutcome::Empty { message } => {
            assert_eq!(message, EMPTY_ANSWER_LENGTH_MESSAGE);
        }
        FinishOutcome::Complete { .. } => panic!("whitespace-only must be Empty"),
    }
}

#[test]
fn truncation_notification_is_none_for_every_non_length_stop_reason() {
    for reason in [
        None,
        Some(StopReason::End),
        Some(StopReason::ToolUse),
        Some(StopReason::Other),
    ] {
        assert!(truncation_notification(ProviderId::OpenAi, reason).is_none());
    }
}

#[test]
fn truncation_notification_points_local_ollama_at_the_control_it_actually_has() {
    let n =
        truncation_notification(ProviderId::Ollama, Some(StopReason::Length)).expect("must warn");
    assert_eq!(n.body, TRUNCATED_ANSWER_LOCAL_MESSAGE);
    assert!(n.body.contains("Max output tokens"));

    let n =
        truncation_notification(ProviderId::OpenAi, Some(StopReason::Length)).expect("must warn");
    assert_eq!(n.body, TRUNCATED_ANSWER_MESSAGE);
    assert!(
        !n.body.contains("Max output tokens"),
        "non-Ollama providers have no such Settings control"
    );
}
