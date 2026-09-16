//! `maxChars` parsing for `answer.assist` — split out of `answer_assist.rs` into its own file (R8
//! relief, PR4: the Prep-tab `topic` addition pushed that module toward the hard LOC cap).
//! Behaviourally identical, only the file it lives in moved; its own tests stayed in the sibling
//! `answer_assist_max_chars_tests.rs` this module still owns via the same `#[path]` convention.

#[cfg(test)]
use serde_json::json;
use serde_json::Value;

use super::answer_assist::{AssistMode, DRAFT_CAP};

/// The picked field's own character limit (`maxChars`, draft mode only —
/// ADR-044 decision 6), read from the DOM by the extension's scan and
/// therefore UNTRUSTED like every other field on this frame.
///
/// Two different bounds meet on this value and they are NOT the same thing.
/// The WIRE bound (`ExtensionAnswerAssistRequestSchema` in
/// `packages/shared/src/ipc/extension-protocol.ts`) pins the SHAPE only, so a
/// well-behaved client cannot send a float or a negative — but a schema is a
/// courtesy, never a guarantee, because this frame arrives over a socket the
/// desktop does not author. The DESKTOP CLAMP is here: anything that is not a
/// positive JSON integer (a float, a string, a negative, zero, a missing key,
/// an older extension that never sends it) reads as "no limit" and leaves the
/// draft path exactly as it was, and an over-large value is reduced to
/// [`DRAFT_CAP`] — the char cap every returned draft is clamped to anyway, so
/// a bigger number could never buy a longer answer. Never an error: a bad
/// limit must degrade to today's behaviour, never refuse a legitimate draft,
/// which is also why the shared TS constant
/// (`EXTENSION_ANSWER_ASSIST_MAX_CHARS`, pinned to [`DRAFT_CAP`] by
/// [`super::test`]) is advertised as a clamp rather than enforced on the wire.
///
/// `mode` is a parameter rather than a call-site `if` so the "rewrite mode
/// IGNORES the field" rule is part of this pure, directly-testable function:
/// a rewrite already carries its own instruction (which may itself ask for a
/// length), and its returned text is never verified against a limit.
///
/// NOT YET WIRED INTO THE DRAFT PATH — hence the `dead_code` allow, which is
/// narrowed to non-test builds so a genuinely orphaned helper still shows up
/// once the caller lands. Stating the limit in the draft prompt and verifying
/// the returned text against it in code (a single re-ask on overshoot) is
/// spec item B1, deliberately deferred: it lands inside the very compose /
/// registry / stream functions PR #1103 rewrites, so it is added on top of
/// that branch's round machinery instead of forking a second copy. Until then
/// the parser and the wire field ship on their own and the feature degrades
/// gracefully — the extension counts the returned text itself.
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

/// Unit tests for [`parse_max_chars`] — split into this sibling file (R8
/// line-budget split), mirroring the existing `#[path]` convention this module inherited from
/// `answer_assist.rs`.
#[cfg(test)]
#[path = "answer_assist_max_chars_tests.rs"]
mod tests;
