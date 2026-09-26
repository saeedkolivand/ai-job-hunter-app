//! The inbound mirror of `agent_call/fence.rs`'s outbound walk: strip a fence wrapper a caller
//! echoed back into a write, before any dispatched command's real body ever sees it.

use serde_json::{json, Value};

use super::super::fence::*;

/// Reverses [`fence_named_fields_recursive`]'s wrapper on every INCOMING
/// `--input` value under a [`FENCE_FIELD_NAMES`] key, before ANY dispatched
/// command's real body ever sees it (security review round 4 — the
/// centralised fix: `commands::scrape::scrape_persist_job`'s own
/// `unfence_job_field` was a hand-added per-call-site strip, and every OTHER
/// freely-dispatchable WRITE command accepting one of these SAME field
/// names had none — three rounds of "add it at the call site" is what
/// produced that gap). A caller that reads a job through a fenced surface
/// (`scrape_list_postings`, `autopilot_list`, `ai_generations_list`, …) and
/// echoes a value straight back into a write would otherwise persist the
/// literal `<job_posting>…</job_posting>` markup into the user's own data —
/// this closes it for every CURRENT and FUTURE writer at the one chokepoint
/// every real dispatch already funnels through ([`dispatch_direct`], called
/// directly for `Read`/`Reversible` and at the tail of
/// [`dispatch_irreversible_confirmed`] for a confirmed `Irreversible`), not
/// one call site at a time. A no-op for the normal case — a clean value
/// that was never fenced — by [`crate::prompt_fence::strip_fence_wrapper`]'s
/// own contract (an exact prefix/suffix match, unchanged otherwise).
/// `commands::scrape::scrape_persist_job`'s own call-site strip is left in
/// place as defense-in-depth at the actual store-write boundary (that
/// command is also reachable from the renderer's normal `invoke()`, not
/// only through this dispatcher) rather than removed.
///
/// Mirrors `fence_named_fields_recursive`'s `ApplicationAnswer` shape rule
/// too (`answers_save` is a real writer of that exact shape), but NOT its
/// [`JOB_RECORD_ANCHOR_FIELDS`] exemption: nothing is written back into a
/// job's `result`, and a strip is a no-op on a value that was never fenced,
/// so the incoming walk stays deliberately unconditional.
pub(in crate::extension_bridge::agent_call) fn unfence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for field in FENCE_FIELD_NAMES {
                // Mirrors the outbound `title`/`body` origin split (issue #1157/#1162): a
                // notification-shaped row fences those two fields under `app_notification`
                // rather than `job_posting` (see `fence.rs`'s `notification_shaped`), so the
                // inbound strip must try both tags. `strip_fence_wrapper` is a no-op on a
                // wrapper for the other tag, so trying both is safe on every other field too.
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let stripped = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                    let stripped =
                        crate::prompt_fence::strip_fence_wrapper("app_notification", &stripped);
                    map.insert((*field).to_string(), json!(stripped));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            let stripped =
                                crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                            *s = crate::prompt_fence::strip_fence_wrapper(
                                "app_notification",
                                &stripped,
                            );
                        }
                    }
                }
            }
            // The mirror of the outbound `text` block's own origin split (issue #1157): `text`
            // is no longer on `FENCE_FIELD_NAMES`, so the loop above never reaches it, but the
            // outbound side can wrap it under EITHER `job_posting` (a non-`DocumentRecord`/
            // `resume_extract_text` producer) or `user_document` (the user's own file) --
            // `strip_fence_wrapper` is a no-op on a value that isn't its own exact wrapper shape,
            // so trying both is safe and never mangles a clean value.
            if let Some(s) = map.get("text").and_then(Value::as_str) {
                let stripped = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                let stripped = crate::prompt_fence::strip_fence_wrapper("user_document", &stripped);
                map.insert("text".to_string(), json!(stripped));
            }
            // The mirror of `fence_named_fields_recursive`'s shape guard:
            // an `ApplicationAnswer`'s `question` goes out fenced, so a
            // caller echoing that record back into a write (`answers_save`)
            // must not persist the markup. Same predicate, same field — see
            // [`APPLICATION_ANSWER_ANCHOR_FIELDS`].
            if is_application_answer_shaped(map) {
                if let Some(question) = map
                    .get(APPLICATION_ANSWER_QUESTION_FIELD)
                    .and_then(Value::as_str)
                {
                    let stripped =
                        crate::prompt_fence::strip_fence_wrapper("job_posting", question);
                    map.insert(
                        APPLICATION_ANSWER_QUESTION_FIELD.to_string(),
                        json!(stripped),
                    );
                }
            }
            for v in map.values_mut() {
                unfence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                unfence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}
