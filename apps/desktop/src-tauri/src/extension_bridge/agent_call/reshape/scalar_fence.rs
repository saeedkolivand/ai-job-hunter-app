//! Fencing a BARE-STRING reply (`agent_call/fence.rs`'s name-keyed walk only reaches a value
//! under a named key) and the truncation-disclosure marker shared with `documents_list` rows.

use serde_json::{json, Value};

/// Commands whose reply is a BARE JSON string carrying untrusted text —
/// [`fence_named_fields_recursive`]'s name-keyed walk only fences a STRING
/// VALUE reached under a named key, so a bare-string reply falls through its
/// `_ => {}` arm untouched no matter how untrusted the text is (issue
/// #1170's follow-up, `B1-r1-ACLI-R5-7`).
///
/// Every dispatchable command returning `String`/`AppResult<String>`,
/// audited (security review round 9, `SEC-1`, after the prior version of
/// this list and comment named only one and claimed — wrongly —
/// completeness): `documents_get_text` (the user's OWN uploaded document
/// text — fenced under the DISTINCT `user_document` tag, MAX_FRAME_BYTES
/// -capped and never silently truncated; see
/// [`USER_DOCUMENT_BARE_TEXT_COMMANDS`]/[`fence_user_document_bare_text`]
/// below, issue #1157/#1162) and `ai_research_answer` (the active
/// provider's own web search results — genuinely third-party text, and the
/// most injection-prone reply on this surface, fenced under `job_posting`
/// with a truncation marker via [`fence_scalar_reply`]'s general arm).
/// `contact_profile_header_line` returns a bare string too, but it is
/// rendered by this app FROM the user's own profile fields, never
/// third-party content the user did not type themselves, so it stays off
/// this list on the same reasoning as
/// `system_get_version`/`system_get_protocol_version` (this app's own
/// values, not user- or third-party-authored text).
const SCALAR_FENCE_COMMANDS: &[&str] = &["documents_get_text", "ai_research_answer"];

/// Fences `data` in place when `command` is on [`SCALAR_FENCE_COMMANDS`] and
/// the reply is actually a bare string — a rename to an object reply (nothing
/// requires `AppResult<String>` to stay that shape) simply stops matching
/// here rather than double-fencing, since [`fence_scraped_fields`]'s
/// name-keyed walk would then cover it instead.
///
/// `documents_get_text` is special-cased out to
/// [`fence_user_document_bare_text`] rather than sharing the generic arm
/// below (issue #1157/#1162): the user's OWN document gets the distinct
/// `user_document` tag and a `MAX_FRAME_BYTES` cap with NO truncation
/// marker — a reply that doesn't fit is refused whole by
/// `super::enforce_frame_cap` rather than silently handed back as a partial
/// document. Every OTHER [`SCALAR_FENCE_COMMANDS`] entry (`ai_research_answer`)
/// takes the generic `job_posting`/[`crate::prompt_fence::JOB_CAP`] path with
/// [`reserve_truncation_marker`], since a caller reading third-party research
/// prose benefits from a usable, honestly-marked prefix more than an outright
/// refusal.
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

/// Commands whose reply is the user's OWN document text as a BARE value, not wrapped in a
/// `text` field on an object -- `fence_named_fields_recursive`'s name-keyed walk (`agent_call/
/// fence.rs`) can only fence a NAMED field, so a command whose whole reply IS the string (no
/// wrapping object at all) needs its own small, audited list here instead (issue #1157).
/// `documents_get_text` returns `AppResult<String>` -- a bare JSON string on success, an object
/// on `Err` (never a string), so this list is command-name keyed rather than shape-keyed the way
/// `fence.rs`'s object shapes are. A SUBSET of [`SCALAR_FENCE_COMMANDS`] above (this dispatch
/// surface's full bare-string coverage list) — the two are not the same list on purpose, since
/// only THIS command's reply is genuinely first-party user content rather than third-party text.
const USER_DOCUMENT_BARE_TEXT_COMMANDS: &[&str] = &["documents_get_text"];

/// Fence `data` under the `user_document` tag when `command` is on
/// [`USER_DOCUMENT_BARE_TEXT_COMMANDS`] and the reply really is a bare string (the success
/// case); a no-op otherwise (an `Err` reply, or any other command). Called from
/// [`fence_scalar_reply`]'s `documents_get_text` special case, itself run from [`fence_reply`]
/// right after -- that walk only ever fences a NAMED field inside an object/array, so it cannot
/// reach a top-level string on its own.
///
/// `pub(super)` (A3-r1-AC-6) -- `agent_call::proof::extract_from_fenced_response` calls
/// [`fence_reply`] the SAME way `dispatch_direct` does (keyed by `ProofSource::read_command()`,
/// the read whose response it is) so a proof's own value and the value a caller actually reads
/// through this dispatcher stay the exact same transform of the exact same read, never two
/// different views of one record.
pub(super) fn fence_user_document_bare_text(command: &str, data: &mut Value) {
    if !USER_DOCUMENT_BARE_TEXT_COMMANDS.contains(&command) {
        return;
    }
    if let Value::String(s) = data {
        // `super::super::MAX_FRAME_BYTES`, not `RESUME_CAP` (issue #1157/#1162 AC-1) --
        // `RESUME_CAP` exists to bound a blob composed INTO a prompt; this is the whole reply to
        // a command whose entire job is "give me my document back", and truncating it here is
        // silent content redaction with nothing on the wire saying so (documents_list.text
        // already carries the JOB_CAP bound as a list row).
        //
        // Not `usize::MAX` either (issue #1183 F6): `fenced`'s cap only bounds how many chars it
        // hands to `neutralize_transcript_boundaries` (~35 patterns), and `usize::MAX` made that
        // pass scan the WHOLE reply -- however large -- before `enforce_frame_cap` (agent_call.rs)
        // ever got a chance to reject it. Capping at `MAX_FRAME_BYTES` chars keeps the fix's own
        // guarantee: every document whose BYTE length fits under `MAX_FRAME_BYTES` has at most
        // that many chars too (a char is never less than a byte), so it passes through this cap
        // completely untouched. A document that doesn't fit gets truncated here only to bound the
        // neutralize pass -- the fence-tag/JSON-envelope overhead this adds still pushes the final
        // reply's BYTE length past `MAX_FRAME_BYTES`, so `enforce_frame_cap` refuses it with
        // `result_too_large` exactly as before, never a silently truncated document on the wire.
        *s = crate::prompt_fence::fenced("user_document", s, super::super::super::MAX_FRAME_BYTES);
    }
}

/// Every fence tag THIS dispatch surface (`agent_call::fence` + this module)
/// can pass to [`crate::prompt_fence::fenced`] — hand-audited the same way
/// [`super::fence::FENCE_FIELD_NAMES`] is, and for the same reason:
/// `prompt_fence::EXPECTED_FENCE_TAGS` pins REGISTRATION crate-wide (most of
/// its entries — `resume_strategy`, `humanize_findings`, … — this surface
/// never emits), so it can't stand in as this surface's own coverage list
/// (A3-r3-AC-3). `mcp::instructions`'s own test asserts `INSTRUCTIONS`
/// documents every tag named here; update THIS list, not just the call
/// site, the moment a new `fenced(...)`/`strip_fence_wrapper(...)` literal
/// tag is added anywhere in `fence.rs` or this module. `ai_research_answer`
/// joining [`SCALAR_FENCE_COMMANDS`] added no new tag here — its generic
/// [`fence_scalar_reply`] arm reuses `job_posting`.
///
/// `#[cfg(test)]` — read only by `mcp::tests`'s coverage assertion, the same
/// reason `agent_call.rs`'s own `dispatch_plan::gate` re-export is gated.
#[cfg(test)]
pub(in crate::extension_bridge) const EMITTED_FENCE_TAGS: [&str; 3] =
    ["job_posting", "user_document", "app_notification"];

/// Trailing marker reserved INSIDE the fence cap when a reply's real text is
/// longer than [`crate::prompt_fence::JOB_CAP`] (`B1-r3-ACLI-R7-5`): the
/// prose deliberately never states the cap NUMBER (a moving implementation
/// detail, not a promise), so without a wire signal a caller has no way to
/// tell "this came back complete" from "this is a prefix" — the app's own
/// "how well do I fit this job" question answered from a silently truncated
/// résumé, or a research answer silently missing its second half. Scoped to
/// every [`SCALAR_FENCE_COMMANDS`] entry OTHER than `documents_get_text`
/// (round-3 fix, M1 — this doc used to name only the `documents_get_text`
/// arm, which stopped being true the moment `ai_research_answer` joined that
/// list; `documents_get_text` itself takes
/// [`fence_user_document_bare_text`]'s own no-silent-truncation path instead)
/// plus [`mark_truncated_document_text`]'s `documents_list` rows below —
/// NOT a change to [`crate::prompt_fence::fenced`] itself, which every OTHER
/// scraped-text surface (job descriptions, autopilot names, …) also calls,
/// where a caller already knows it is reading third-party board text, not
/// first-party output.
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

/// Pre-fence step for `documents_list`: reserves [`TRUNCATION_MARKER`] room in
/// every row's `text` field BEFORE the generic [`fence_scraped_fields`] walk
/// truncates it at the cap — that walk is keyed by FIELD NAME, not command
/// (security review round 2's deliberate design; see `FENCE_FIELD_NAMES`'s
/// own doc), so this runs ahead of it rather than teaching the shared,
/// security-critical walk one command's own truncation-disclosure policy.
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
