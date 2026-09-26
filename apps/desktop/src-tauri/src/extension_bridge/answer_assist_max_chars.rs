//! `maxChars` parsing for `answer.assist` — split out of `answer_assist.rs` into its own file (R8
//! relief, PR4: the Prep-tab `topic` addition pushed that module toward the hard LOC cap).
//! Behaviourally identical, only the file it lives in moved; its own tests live in the sibling
//! `answer_assist_max_chars/tests.rs`.

#[cfg(test)]
use serde_json::json;
use serde_json::Value;

use super::answer_assist::{AssistMode, DRAFT_CAP};

/// The picked field's own character limit (`maxChars`, draft mode only —
/// ADR-044 decision 6), read from the DOM by the extension's scan and
/// therefore UNTRUSTED like every other field on this frame.
///
/// The wire schema (`ExtensionAnswerAssistRequestSchema`) pins the SHAPE
/// only — a schema is a courtesy, never a guarantee, since this frame
/// arrives over a socket the desktop does not author. The DESKTOP CLAMP is
/// here: anything that is not a positive JSON integer reads as "no limit"
/// (leaves the draft path unchanged), and an over-large value is reduced to
/// [`DRAFT_CAP`] — the cap every returned draft is clamped to anyway, so a
/// bigger number could never buy a longer answer. Never an error: a bad
/// limit degrades to today's behaviour, never refuses a legitimate draft.
///
/// `mode` is a parameter (not a call-site `if`) so "rewrite mode IGNORES the
/// field" is part of this pure, directly-testable function.
///
/// NOT YET WIRED INTO THE DRAFT PATH — hence the `dead_code` allow (narrowed
/// to non-test builds). Stating the limit in the draft prompt and verifying
/// the returned text against it is spec item B1, deliberately deferred onto
/// the compose/registry/stream round machinery PR #1103 rewrites rather than
/// forking a second copy. Until then the parser ships on its own and the
/// feature degrades gracefully — the extension counts the returned text
/// itself.
#[cfg_attr(not(test), allow(dead_code))]
fn parse_max_chars(payload: &Value, mode: AssistMode) -> Option<usize> {
    if mode != AssistMode::Draft {
        return None;
    }
    let requested = payload.get("maxChars")?.as_u64()?;
    if requested == 0 {
        return None;
    }
    Some(
        usize::try_from(requested)
            .unwrap_or(DRAFT_CAP)
            .min(DRAFT_CAP),
    )
}

/// Unit tests for [`parse_max_chars`] — see `tests.rs`.
#[cfg(test)]
mod tests;
