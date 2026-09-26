//! The name-keyed recursive fence walk `fence_scraped_fields` (`agent_call/fence.rs`) runs.

use serde_json::{json, Value};

use super::shape_helpers::{
    fence_all_string_leaves, fence_board_derived_strings, fence_scrape_summaries_recursive,
};
use super::shape_tables::{
    is_application_answer_shaped, APPLICATION_ANSWER_QUESTION_FIELD, JOB_RECORD_ANCHOR_FIELDS,
    JOB_RECORD_RESULT_FIELD,
};
use super::tables::{
    CHANGELOG_ENTRY_ANCHOR_FIELDS, DOCUMENT_RECORD_ANCHOR_FIELDS, FENCE_FIELD_NAMES,
    JOB_POSTING_ANCHOR_FIELDS, JOB_POSTING_SAFE_FIELDS, NOTIFICATION_ANCHOR_FIELDS,
    RESUME_EXTRACT_TEXT_ANCHOR_FIELD,
};

/// Walk every object/array in `value`, fencing any [`FENCE_FIELD_NAMES`]
/// STRING key (or string element of an ARRAY under one of those keys)
/// wherever one appears, then — on an object [`JOB_POSTING_ANCHOR_FIELDS`]
/// marks as a real `JobPosting` — every OTHER string-valued key not in
/// [`JOB_POSTING_SAFE_FIELDS`] (the flattened `extra` catch-all). See
/// [`fence_scraped_fields`]'s doc for why this is recursive and
/// unconditional.
///
/// Then the shape rules: on an [`APPLICATION_ANSWER_ANCHOR_FIELDS`]-
/// detected object the [`APPLICATION_ANSWER_QUESTION_FIELD`] string is
/// fenced (a scraped ATS question label whose wire key is shared with this
/// app's own `InterviewQuestion.question`), on a scrape-diagnostics object
/// [`fence_board_derived_strings`] fences the board-written keys, and on a
/// [`JOB_RECORD_ANCHOR_FIELDS`]-detected object the recursion hands
/// [`JOB_RECORD_RESULT_FIELD`] to [`fence_scrape_summaries_recursive`]
/// instead of walking it (a job's own output, not scraped text — except for
/// the diagnostics a scrape completes with).
pub(in crate::extension_bridge::agent_call) fn fence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Issue #1157 — origin shape checks, computed up front (read-only) so the loop below
            // and the dedicated `text` block after it share one derivation. Every flag below is
            // ANDed with `!job_posting_shaped` (rounds A3-r1/2, issue #1183 F1): a board-controlled
            // `JobPosting.extra` (`#[serde(flatten)]`) could otherwise forge the anchor keys of any
            // OTHER shape (`confidence`, `publishedAt`+`prerelease`, `createdAt`+`read`) and steal
            // that shape's exemption/relabelling for board-authored text — a real `JobPosting`
            // always fails the AND, so its own fields fence exactly as they always did.
            let job_posting_shaped = JOB_POSTING_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            let document_record_shaped = !job_posting_shaped
                && DOCUMENT_RECORD_ANCHOR_FIELDS
                    .iter()
                    .all(|f| map.contains_key(*f));
            let user_document_shaped = !job_posting_shaped
                && (document_record_shaped || map.contains_key(RESUME_EXTRACT_TEXT_ANCHOR_FIELD));
            let changelog_entry_shaped = !job_posting_shaped
                && CHANGELOG_ENTRY_ANCHOR_FIELDS
                    .iter()
                    .all(|f| map.contains_key(*f));
            let notification_shaped = !job_posting_shaped
                && NOTIFICATION_ANCHOR_FIELDS
                    .iter()
                    .all(|f| map.contains_key(*f));

            for field in FENCE_FIELD_NAMES {
                // `title` on a `DocumentRecord`-shaped object is the user's own first-party
                // file title, not a board-scraped job title -- skip the default fence. Gated on
                // `document_record_shaped` alone, which already excludes a `JobPosting` (see
                // above).
                if *field == "title" && document_record_shaped {
                    continue;
                }
                // `body` on a changelog-entry-shaped object is this repo's own first-party
                // release notes -- skip the default fence.
                if *field == "body" && changelog_entry_shaped {
                    continue;
                }
                // A3-r2-AC-7: `title`/`body` on a notification-shaped object stay FENCED (mixed
                // provenance, never skipped the way the two exemptions above are), just under
                // the distinct `app_notification` tag rather than `job_posting`'s
                // third-party-board-authorship claim.
                let tag = if (*field == "title" || *field == "body") && notification_shaped {
                    "app_notification"
                } else {
                    "job_posting"
                };
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let fenced = crate::prompt_fence::fenced(tag, s, crate::prompt_fence::JOB_CAP);
                    map.insert((*field).to_string(), json!(fenced));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            *s = crate::prompt_fence::fenced(tag, s, crate::prompt_fence::JOB_CAP);
                        }
                    }
                }
            }
            // `text` (issue #1157) -- origin-aware, never a flat `FENCE_FIELD_NAMES` entry: the
            // user's OWN document text (`documents::DocumentRecord.text`/`resume_extract_text`'s
            // reply) is fenced under the DISTINCT `user_document` tag; every other producer on
            // this surface (`commands::profile_import::profile_import_from_url`'s response also
            // carries a bare `text` key, but it is resume text rendered from a THIRD-PARTY
            // imported profile page, not the user's own file) keeps the ORIGINAL `job_posting`
            // default -- a shape miss must stay fenced, never fall open.
            if let Some(s) = map.get("text").and_then(Value::as_str) {
                let (tag, cap) = if user_document_shaped {
                    ("user_document", crate::prompt_fence::RESUME_CAP)
                } else {
                    ("job_posting", crate::prompt_fence::JOB_CAP)
                };
                let fenced = crate::prompt_fence::fenced(tag, s, cap);
                map.insert("text".to_string(), json!(fenced));
            }
            // `title`/`name` (security review round A3-r1, SEC-4 MEDIUM): a `DocumentRecord`'s
            // `title` is exempted from the default fence above, and its `name` was never on
            // [`FENCE_FIELD_NAMES`] at all -- both are agent-WRITABLE (`documents_import`) and
            // agent-READABLE strings that, unlike every other exempted first-party field on this
            // surface, had NO cap and NO boundary defence left at all. Neutralize + cap without a
            // tag (the same treatment `agent_read::found_jobs::cap_autopilot_name` gives an
            // autopilot's own name): the first-party voice stays unlabelled, but a stored
            // prompt-injection payload can no longer forge a transcript boundary or grow
            // unbounded through this channel.
            if document_record_shaped {
                for name_field in ["title", "name"] {
                    if let Some(s) = map.get(name_field).and_then(Value::as_str) {
                        let capped: String = s.chars().take(crate::prompt_fence::JOB_CAP).collect();
                        let capped = crate::prompt_fence::neutralize_transcript_boundaries(&capped);
                        map.insert(name_field.to_string(), json!(capped));
                    }
                }
            }
            // Keys the `extra` catch-all above already fenced leaf-by-leaf (test-author round,
            // TR-02) — the trailing recursion below must skip them, or an Array/Object `extra` key
            // whose OWN inner key is ALSO on `FENCE_FIELD_NAMES` (e.g. `"salaryDetail":
            // {"description": …}`) gets double-wrapped, leaving a stray wrapper after
            // `unfence_named_fields_recursive`'s single strip.
            let mut extra_fenced_keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            if job_posting_shaped {
                // Collects every non-null, non-safe, non-listed `extra` key regardless of shape —
                // not `v.is_string()` alone (security review round 4): an Array/Object-valued
                // board-chosen key (not reachable today — every `extra.insert` call site writes a
                // scalar, verified — but not reachable isn't impossible for the first board that
                // adds one) would otherwise skip every fencing path here, since the generic
                // recursion below only fences NAMED fields. A String fences directly; an
                // Array/Object fences leaf-by-leaf via `fence_all_string_leaves`.
                let extra_keys: Vec<String> = map
                    .iter()
                    .filter(|(k, v)| {
                        !v.is_null()
                            && !FENCE_FIELD_NAMES.contains(&k.as_str())
                            // `text` is no longer on `FENCE_FIELD_NAMES` (issue #1157 -- it is
                            // fenced by its own origin-aware block above this shape check,
                            // unconditionally); excluded here too so a `JobPosting`'s own `text`
                            // key (if a board ever added one to its `extra`) is never fenced
                            // TWICE under two different tags.
                            && *k != "text"
                            && !JOB_POSTING_SAFE_FIELDS.contains(&k.as_str())
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                for key in extra_keys {
                    if let Some(v) = map.get_mut(&key) {
                        match v {
                            Value::String(s) => {
                                *s = crate::prompt_fence::fenced(
                                    "job_posting",
                                    s,
                                    crate::prompt_fence::JOB_CAP,
                                );
                            }
                            Value::Array(_) | Value::Object(_) => fence_all_string_leaves(v),
                            _ => {}
                        }
                    }
                    extra_fenced_keys.insert(key);
                }
            }
            // Shape-guarded, never a name entry — see
            // [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for why putting
            // `question` on [`FENCE_FIELD_NAMES`] would have re-fenced
            // `InterviewQuestion.question`. Skipped on a `JobPosting`-shaped
            // object: the catch-all above already fenced every unclassified
            // string there, and fencing twice would leave a wrapper behind
            // after [`unfence_named_fields_recursive`]'s single strip.
            if !job_posting_shaped && is_application_answer_shaped(map) {
                if let Some(question) = map
                    .get(APPLICATION_ANSWER_QUESTION_FIELD)
                    .and_then(Value::as_str)
                {
                    let fenced = crate::prompt_fence::fenced(
                        "job_posting",
                        question,
                        crate::prompt_fence::JOB_CAP,
                    );
                    map.insert(APPLICATION_ANSWER_QUESTION_FIELD.to_string(), json!(fenced));
                }
            }
            // Same `!job_posting_shaped` guard, same double-wrap reason as above (`fenced`/
            // `fence_all_string_leaves` don't guard against it). Reached by
            // `Autopilot.last_run_summaries`; the copies inside a `JobRecord`'s exempt `result`
            // are handled below.
            if !job_posting_shaped {
                fence_board_derived_strings(map);
            }
            // A `JobRecord`'s own `result` is the app's OWN output, not
            // scraped text — see [`JOB_RECORD_ANCHOR_FIELDS`]. The exemption
            // is on the RECURSION only: every other field of this object,
            // and every other object in the tree, walks as before.
            let job_record_shaped = JOB_RECORD_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            for (key, v) in map.iter_mut() {
                if extra_fenced_keys.contains(key.as_str()) {
                    // Already fenced leaf-by-leaf by the `extra` catch-all above -- see TR-02
                    // fix note there. Re-walking here would double-wrap any inner key that also
                    // happens to be on [`FENCE_FIELD_NAMES`].
                    continue;
                }
                if job_record_shaped && key.as_str() == JOB_RECORD_RESULT_FIELD {
                    // The exemption is wholesale for the NAME-keyed walk, and
                    // stays that way — but `scrape_boards` completes with
                    // `BoardScrapeSummary` rows, so a diagnostics shape does
                    // carry third-party text in here. Fence only those
                    // enumerated keys and nothing else in the subtree; see
                    // [`SCRAPE_SUMMARY_ANCHOR_FIELDS`].
                    fence_scrape_summaries_recursive(v);
                    continue;
                }
                fence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}
