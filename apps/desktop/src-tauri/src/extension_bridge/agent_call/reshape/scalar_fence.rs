//! Fencing a BARE-STRING reply (`agent_call/fence.rs`'s name-keyed walk only reaches a value
//! under a named key) and the truncation-disclosure marker shared with `documents_list` rows.

use serde_json::{json, Value};

/// Commands whose reply is a BARE JSON string carrying untrusted text —
/// [`fence_named_fields_recursive`]'s name-keyed walk only fences a STRING VALUE reached under a
/// named key, so a bare-string reply falls through untouched no matter how untrusted (issue
/// #1170's follow-up).
///
/// Every dispatchable command returning `String`/`AppResult<String>`, audited (security review
/// round 9): `documents_get_text` (the user's OWN uploaded document text — fenced under the
/// DISTINCT `user_document` tag, MAX_FRAME_BYTES-capped and never silently truncated; see
/// [`USER_DOCUMENT_BARE_TEXT_COMMANDS`]/[`fence_user_document_bare_text`] below) and
/// `ai_research_answer` (the active provider's own web search results — genuinely third-party, the
/// most injection-prone reply on this surface, fenced under `job_posting` with a truncation marker
/// via [`fence_scalar_reply`]'s general arm). `contact_profile_header_line` also returns a bare
/// string, but it is rendered by this app FROM the user's own profile fields — never third-party
/// content — same reasoning as `system_get_version`/`system_get_protocol_version`.
const SCALAR_FENCE_COMMANDS: &[&str] = &["documents_get_text", "ai_research_answer"];

/// Fences `data` in place when `command` is on [`SCALAR_FENCE_COMMANDS`] and the reply is actually
/// a bare string — a rename to an object reply simply stops matching here rather than
/// double-fencing, since [`fence_scraped_fields`]'s name-keyed walk would then cover it instead.
///
/// `documents_get_text` is special-cased to [`fence_user_document_bare_text`] rather than sharing
/// the generic arm: the user's OWN document gets the distinct `user_document` tag and a
/// `MAX_FRAME_BYTES` cap with NO truncation marker — an oversized reply is refused whole by
/// `super::enforce_frame_cap` rather than silently handed back partial. Every OTHER entry
/// (`ai_research_answer`) takes the generic `job_posting` path with [`reserve_truncation_marker`],
/// since a caller reading third-party research prose benefits more from a usable, honestly-marked
/// prefix than an outright refusal.
pub(super) fn fence_scalar_reply(command: &str, data: &mut Value) {
    if command == "documents_get_text" {
        fence_user_document_bare_text(command, data);
        return;
    }
    if let Value::String(s) = data {
        if SCALAR_FENCE_COMMANDS.contains(&command) {
            let marked = reserve_truncation_marker(s, crate::prompt_fence::JOB_CAP);
            *data = json!(crate::prompt_fence::fenced(
                "job_posting",
                &marked,
                crate::prompt_fence::JOB_CAP
            ));
        }
    }
}

/// Commands whose reply is the user's OWN document text as a BARE value, not wrapped in a `text`
/// field — `fence_named_fields_recursive`'s name-keyed walk can only fence a NAMED field, so a
/// whole-reply-IS-the-string command needs its own small, audited list here (issue #1157). A
/// SUBSET of [`SCALAR_FENCE_COMMANDS`] above on purpose: only THIS command's reply is genuinely
/// first-party user content rather than third-party text.
const USER_DOCUMENT_BARE_TEXT_COMMANDS: &[&str] = &["documents_get_text"];

/// Fence `data` under the `user_document` tag when `command` is on
/// [`USER_DOCUMENT_BARE_TEXT_COMMANDS`] and the reply really is a bare string; a no-op otherwise.
/// Called from [`fence_scalar_reply`]'s `documents_get_text` special case, since the name-keyed
/// walk cannot reach a top-level string on its own.
///
/// `pub(super)` (A3-r1-AC-6) — `proof::extract_from_fenced_response` calls [`fence_reply`] the
/// SAME way `dispatch_direct` does, so a proof's own value and the value a caller reads stay the
/// exact same transform of the exact same read.
pub(super) fn fence_user_document_bare_text(command: &str, data: &mut Value) {
    if !USER_DOCUMENT_BARE_TEXT_COMMANDS.contains(&command) {
        return;
    }
    if let Value::String(s) = data {
        // `MAX_FRAME_BYTES`, not `RESUME_CAP` (issue #1157/#1162 AC-1) -- `RESUME_CAP` bounds a
        // blob composed INTO a prompt, while this is the whole reply to "give me my document
        // back", so truncating at that smaller cap would be silent content redaction.
        //
        // Not `usize::MAX` either (issue #1183 F6): `fenced`'s cap bounds how many chars
        // `neutralize_transcript_boundaries` scans, and `usize::MAX` let that pass scan the WHOLE
        // reply before `enforce_frame_cap` ever got a chance to reject it. `MAX_FRAME_BYTES` chars
        // keeps the guarantee that a document under that many BYTES passes through untouched,
        // while one that doesn't fit still gets refused by `enforce_frame_cap` with
        // `result_too_large`, never silently truncated on the wire.
        *s = crate::prompt_fence::fenced("user_document", s, super::super::super::MAX_FRAME_BYTES);
    }
}

/// Every fence tag THIS dispatch surface can pass to [`crate::prompt_fence::fenced`] —
/// hand-audited, since `prompt_fence::EXPECTED_FENCE_TAGS` pins registration crate-wide (most
/// entries this surface never emits) and can't stand in as this surface's own coverage list.
/// `mcp::instructions`'s own test asserts `INSTRUCTIONS` documents every tag named here; update
/// THIS list the moment a new fence-tag literal is added anywhere in `fence.rs` or this module.
///
/// `#[cfg(test)]` — read only by `mcp::tests`'s coverage assertion.
#[cfg(test)]
pub(in crate::extension_bridge) const EMITTED_FENCE_TAGS: [&str; 3] =
    ["job_posting", "user_document", "app_notification"];

/// Trailing marker reserved INSIDE the fence cap when a reply's real text is longer than
/// [`crate::prompt_fence::JOB_CAP`]: the prose never states the cap NUMBER, so without a wire
/// signal a caller can't tell "complete" from "prefix" — silently truncated résumé or research
/// answer. Scoped to every [`SCALAR_FENCE_COMMANDS`] entry OTHER than `documents_get_text` (which
/// takes [`fence_user_document_bare_text`]'s own no-silent-truncation path instead) plus
/// [`mark_truncated_document_text`]'s `documents_list` rows — NOT a change to
/// [`crate::prompt_fence::fenced`] itself, used elsewhere where a caller already knows it's
/// reading third-party board text.
pub(in crate::extension_bridge::agent_call) const TRUNCATION_MARKER: &str =
    "\n[TRUNCATED — longer than the fence cap; this is a prefix, not the whole reply]";

/// Truncates `body` to `cap` chars the same way [`crate::prompt_fence::fenced`]
/// itself will, but reserves room for [`TRUNCATION_MARKER`] and appends it —
/// so the marker always survives inside the cap rather than being cut off by
/// `fenced`'s own truncation. A `body` already within `cap` chars is
/// returned unchanged, and `fenced` then wraps it as a no-op truncation, so
/// the marker never appears on a document that was never cut.
pub(in crate::extension_bridge::agent_call) fn reserve_truncation_marker(
    body: &str,
    cap: usize,
) -> String {
    if body.chars().count() <= cap {
        return body.to_string();
    }
    let budget = cap.saturating_sub(TRUNCATION_MARKER.chars().count());
    let truncated: String = body.chars().take(budget).collect();
    format!("{truncated}{TRUNCATION_MARKER}")
}

/// Pre-fence step for `documents_list`: reserves [`TRUNCATION_MARKER`] room in every row's `text`
/// field BEFORE the generic [`fence_scraped_fields`] walk truncates it at the cap — that walk is
/// keyed by FIELD NAME, not command (deliberate design; see `FENCE_FIELD_NAMES`'s own doc), so
/// this runs ahead of it rather than teaching the shared walk one command's own policy.
pub(in crate::extension_bridge::agent_call) fn mark_truncated_document_text(data: &mut Value) {
    let Value::Array(rows) = data else { return };
    for row in rows.iter_mut() {
        let Value::Object(map) = row else { continue };
        let Some(text) = map.get("text").and_then(Value::as_str) else {
            continue;
        };
        let marked = reserve_truncation_marker(text, crate::prompt_fence::JOB_CAP);
        if marked != text {
            map.insert("text".to_string(), json!(marked));
        }
    }
}
