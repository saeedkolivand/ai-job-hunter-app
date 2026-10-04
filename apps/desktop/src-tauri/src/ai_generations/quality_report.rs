//! The `quality_report` column's write-boundary guard and its per-key merge.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Nothing here touches SQLite: the byte cap, the
//! sentinel-drop both untrusted write paths share, and the wrapper merge a
//! per-job save runs.

/// A direct IPC caller or a hostile/malformed backup bundle (the `import`
/// path below) could otherwise hand this column an unbounded blob.
///
/// This caps the WRAPPER (`{schemaVersion, pipeline, generatedAt, resume?,
/// coverLetter?}`), not one sub-report — the wrapper legitimately holds TWO
/// of them (résumé + cover letter), each up to its own documented worst
/// case.
///
/// **The arithmetic is on SERIALIZED bytes, not raw field bytes** (PR #963
/// round 14 fix — the prior 512 KiB sizing used only the RAW arithmetic
/// below and could silently drop a legitimate quote-heavy report). Per
/// `validate::content::ISSUE_MESSAGE_MAX_BYTES`'s doc, one sub-report's RAW
/// (pre-serialization) `message`/`evidence`/`section` content tops out
/// around `MAX_CONTENT_ISSUES` (200) × (400 + 400 + 120 + ~150 bytes of JSON
/// overhead) ≈ 214 KB — but `serde_json` ESCAPES every byte it writes: a
/// `"` becomes `\"` and a `\` becomes `\\` (2× that byte), a raw control
/// char becomes `\u00XX` (6×). Quoted résumé bullets or code-snippet
/// evidence routinely hit that 2× density on the three clamped text fields;
/// this cap assumes AT MOST 2× escape density there (not the pathological
/// 6× control-char case, which generated document text does not carry, and
/// which a byte cap alone cannot close anyway — `sanitize_quality_report`'s
/// existing sentinel-drop already degrades that case safely, just more
/// eagerly). That puts one sub-report's SERIALIZED worst case near 389 KB
/// (200 × ((400×2) + (400×2) + (120×2) + ~150)), two of them near 777 KB,
/// and the full wrapper — plus the envelope fields and the per-slot
/// `sourceTextHash`es (well under 100 KB combined) — near 878 KB. 1 MiB
/// (exactly 2× the prior 512 KiB) covers that ≈878 KB escaped worst case
/// with ~14% headroom to spare, without silently reverting to the raw-only
/// arithmetic the round-6 regression test never actually exercised (it
/// filled the worst-case blob with plain, non-escaping `'x'` characters —
/// see `sanitize_quality_report_keeps_a_two_sub_report_wrapper_at_escaped_worst_case_size`
/// in `tests/report_cap.rs` for the corrected, measured version).
///
/// **`MAX_CONTENT_ISSUES + 1` is the assumption, and it is enforced by the
/// one capper.** The `+ 1` is `report.truncated`, and there is exactly one of
/// it however many times a report is capped. That matters because a report is
/// not written once: at max depth `pipeline::resume::stages::judge` merges up
/// to `MAX_JUDGE_ITEMS` (6) advisory Warnings into an already-capped list
/// AFTER `validate` produced it, which put this derivation's own bound out by
/// six on every max run until the merge started re-applying
/// `validate::content::cap_issues`. Any future stage that appends to
/// `ContentReport::issues` owes the same call.
///
/// Dropping per-slot instead of the whole wrapper (persist whichever
/// sub-report fits, drop only the other) was considered and rejected: it's
/// real machinery (partial-parse, partial-merge, a partial-drop log) for a
/// defect that a correctly-sized flat cap already closes.
///
/// The single source of truth for both write paths that persist
/// `quality_report` (`commands::ai_generations::ai_generations_save` and
/// [`AiGenerationStore::import`] below) — the data layer owns the column, so
/// the cap lives here rather than duplicated per caller.
pub(super) const QUALITY_REPORT_MAX_BYTES: usize = 1024 * 1024;

/// Guard an incoming `quality_report` blob against exceeding
/// [`QUALITY_REPORT_MAX_BYTES`] on write. A byte-position clamp
/// (`clamp_to_bytes`) is wrong here: JSON truncated mid-object is
/// unparseable, and [`merge_quality_report`]'s first guard would then
/// silently keep `existing` instead of the fresh report — same outcome as
/// the documented "no report" sentinel, but reached by parse accident
/// instead of by design, with no signal that a report was dropped. This
/// substitutes the sentinel (`""`) outright and logs a content-free warning
/// (source + byte count only, per ADR-027 — never report content) so the
/// drop is diagnosable. Shared by both untrusted write paths for this
/// column: `commands::ai_generations::ai_generations_save` and
/// [`AiGenerationStore::import`] below.
pub(crate) fn sanitize_quality_report(report: String, source: &str) -> String {
    if report.len() > QUALITY_REPORT_MAX_BYTES {
        log::warn!(
            "[ai_generations] {source}: quality_report exceeded {QUALITY_REPORT_MAX_BYTES} \
             bytes ({} bytes); dropped to the empty sentinel — merge_quality_report keeps \
             the existing report by design",
            report.len()
        );
        return String::new();
    }
    report
}

/// Merge two `quality_report` wrapper blobs — `{schemaVersion, pipeline,
/// generatedAt, resume?, coverLetter?}` (renderer-owned shape; each sub-report
/// may itself carry a `sourceTextHash`, see the field doc on
/// [`super::AiGenerationRecord::quality_report`]) — by TOP-LEVEL key, so a
/// letter-only save updates only `coverLetter` (plus the envelope fields it
/// always carries) and a résumé-only save updates only `resume`, without
/// wiping whichever sub-report the OTHER save last wrote.
///
/// - `incoming` empty or not a parseable JSON object (a content-less save, or
///   the migration's backfilled `''`) → `existing` is kept verbatim.
/// - `existing` not a parseable JSON object but `incoming` is (nothing to
///   overlay a key onto) → `incoming` wins outright.
/// - Otherwise: every top-level key `incoming` carries — including
///   `schemaVersion`/`pipeline`/`generatedAt` — overlays the same key on
///   `existing`; every key only `existing` carries survives untouched.
/// - The merged object exceeds [`QUALITY_REPORT_MAX_BYTES`] (both write paths
///   run `incoming` alone through [`sanitize_quality_report`] first, but the
///   sub-report `existing` already carries for the OTHER pipeline can
///   independently sit near the cap too, so their union can still blow the
///   column budget) → `incoming` is returned whole instead. At that point
///   `incoming` is known-parseable (it passed the first guard above) and
///   known within budget (both callers sanitize it before it reaches here);
///   that beats both alternatives — truncating the merged JSON mid-object
///   would make it unparseable on the next read and silently revert to the
///   stale stored report, and persisting the oversized union would blow the
///   column budget outright.
pub(super) fn merge_quality_report(incoming: String, existing: String) -> String {
    let Some(serde_json::Value::Object(incoming_obj)) =
        serde_json::from_str::<serde_json::Value>(&incoming).ok()
    else {
        return existing;
    };
    let Some(serde_json::Value::Object(mut merged)) =
        serde_json::from_str::<serde_json::Value>(&existing).ok()
    else {
        return incoming;
    };
    for (key, value) in incoming_obj {
        merged.insert(key, value);
    }
    match serde_json::to_string(&serde_json::Value::Object(merged)) {
        Ok(merged_str) if merged_str.len() <= QUALITY_REPORT_MAX_BYTES => merged_str,
        Ok(merged_str) => {
            // Same event class as `sanitize_quality_report`'s drop, so it gets
            // the same content-free signal (byte counts only, ADR-027) — this
            // path discards the OTHER document's stored sub-report.
            log::warn!(
                "[ai_generations] merge_quality_report: merged union exceeded \
                 {QUALITY_REPORT_MAX_BYTES} bytes ({} bytes; incoming {}); keeping the \
                 incoming report whole and discarding the stored other-document sub-report",
                merged_str.len(),
                incoming.len()
            );
            incoming
        }
        Err(_) => incoming,
    }
}
