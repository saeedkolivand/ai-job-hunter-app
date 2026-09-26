//! The shape-fencing walkers `fence_named_fields_recursive` delegates to for scrape-diagnostics
//! shapes and for an unclassified `JobPosting.extra` value.

use serde_json::{json, Value};

use super::shape_tables::{
    BOARD_HEALTH_ANCHOR_FIELDS, BOARD_HEALTH_UNTRUSTED_FIELDS, SCRAPE_SUMMARY_ANCHOR_FIELDS,
    SCRAPE_SUMMARY_UNTRUSTED_FIELDS,
};

/// Fence the board-written strings on `map` when its keys match either
/// scrape-diagnostics shape — [`SCRAPE_SUMMARY_ANCHOR_FIELDS`] →
/// [`SCRAPE_SUMMARY_UNTRUSTED_FIELDS`], [`BOARD_HEALTH_ANCHOR_FIELDS`] →
/// [`BOARD_HEALTH_UNTRUSTED_FIELDS`] — and nothing at all on any other
/// object. The two shapes are checked independently rather than nested: a
/// `BoardHealth` also reaches this surface standalone, on a
/// `BoardHealthEntry`, not only under a summary's `health`.
///
/// Shared by [`fence_named_fields_recursive`] (diagnostics anywhere OUTSIDE
/// a job result) and [`fence_scrape_summaries_recursive`] (the copies INSIDE
/// the otherwise-exempt one), so the two walks can never disagree about
/// either shape or either field set.
///
/// Fencing happens on this READ path rather than at the producer
/// (`commands::scrape::scrape_boards`, before `job_complete`) on purpose:
/// the very same strings are what the renderer's per-board chip strip
/// displays — `BoardSummaryChips` matches `skipped` against a controlled
/// vocabulary to pick a localized label, and renders `error`, `truncated`
/// and `health.lastError` as chip detail — reached both by the
/// `job.completed` event and, on remount, by the watchdog's own `jobs_get`.
/// A fence baked into the stored result would put `<job_posting>` markup on
/// screen and knock `skipped` out of every arm of that match; stripping it
/// back off in the renderer would mean a second, hand-maintained copy of
/// these field lists in TypeScript, on a path where a miss is visible to the
/// user.
pub(in crate::extension_bridge::agent_call) fn fence_board_derived_strings(
    map: &mut serde_json::Map<String, Value>,
) {
    for (anchors, fields) in [
        (
            SCRAPE_SUMMARY_ANCHOR_FIELDS.as_slice(),
            SCRAPE_SUMMARY_UNTRUSTED_FIELDS.as_slice(),
        ),
        (
            BOARD_HEALTH_ANCHOR_FIELDS.as_slice(),
            BOARD_HEALTH_UNTRUSTED_FIELDS.as_slice(),
        ),
    ] {
        if !anchors.iter().all(|f| map.contains_key(*f)) {
            continue;
        }
        for field in fields {
            if let Some(s) = map.get(*field).and_then(Value::as_str) {
                let fenced =
                    crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
                map.insert((*field).to_string(), json!(fenced));
            }
        }
    }
}

/// Walk `value` applying ONLY [`fence_board_derived_strings`] — the single
/// carve-out inside a `JobRecord`'s otherwise-exempt
/// [`JOB_RECORD_RESULT_FIELD`]. Deliberately NOT
/// [`fence_named_fields_recursive`]: running the name-keyed walk in here
/// would re-open the exact defect the exemption exists to close (a
/// generation's `{"done": true, "text": …}` labelled as a scraped posting).
/// A scrape summary and its board health are fenced; everything else in the
/// subtree is left exactly as the producer wrote it.
pub(in crate::extension_bridge::agent_call) fn fence_scrape_summaries_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            fence_board_derived_strings(map);
            for v in map.values_mut() {
                fence_scrape_summaries_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_scrape_summaries_recursive(item);
            }
        }
        _ => {}
    }
}

/// Fence every STRING found anywhere inside `value`, unconditionally — no
/// field-name gate, unlike [`fence_named_fields_recursive`]. Used only for a
/// value already known to be untrusted board data by virtue of its
/// LOCATION (an unclassified key under a detected `JobPosting`'s flattened
/// `extra`), so every string it contains, at any depth, is untrusted too —
/// the board chose the keys, so a name-based allowlist can never enumerate
/// them.
pub(in crate::extension_bridge::agent_call) fn fence_all_string_leaves(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_all_string_leaves(item);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                fence_all_string_leaves(v);
            }
        }
        _ => {}
    }
}
