use super::{support::*, *};

/// Unit coverage for the shared guard both write paths call through.
#[test]
fn sanitize_quality_report_drops_an_over_cap_report_to_the_empty_string() {
    let oversized = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "letter",
        "generatedAt": 1,
        "coverLetter": { "blob": "x".repeat(QUALITY_REPORT_MAX_BYTES) }
    })
    .to_string();
    assert!(oversized.len() > QUALITY_REPORT_MAX_BYTES);
    assert_eq!(sanitize_quality_report(oversized, "test"), "");
}

#[test]
fn sanitize_quality_report_leaves_an_in_budget_report_untouched() {
    let small = r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":1,"resume":{"ok":true}}"#;
    assert_eq!(sanitize_quality_report(small.into(), "test"), small);
}

/// Regression for the finding this fix closes: `QUALITY_REPORT_MAX_BYTES`
/// guards the WRAPPER, which legitimately holds two sub-reports (résumé +
/// cover letter), not one. Builds two synthetic sub-reports at the exact
/// worst-case byte count `validate::content::ISSUE_MESSAGE_MAX_BYTES`'s doc
/// computes per sub-report (`MAX_CONTENT_ISSUES` issues × the three
/// clamped-field byte caps + JSON overhead) and asserts the resulting
/// two-sub-report wrapper survives `sanitize_quality_report` byte-for-byte —
/// the exact document the cap is sized to allow, not silently dropped to the
/// empty sentinel.
#[test]
fn sanitize_quality_report_keeps_a_two_sub_report_wrapper_at_worst_case_size() {
    use crate::validate::content::{
        ISSUE_EVIDENCE_MAX_BYTES, ISSUE_MESSAGE_MAX_BYTES, ISSUE_SECTION_MAX_BYTES,
        MAX_CONTENT_ISSUES,
    };
    // Mirrors `ISSUE_MESSAGE_MAX_BYTES`'s doc arithmetic (≈214 KB per
    // sub-report). The `~150` JSON-overhead term there is deliberately
    // approximate (not a named constant), so this pads past it slightly to
    // stay a genuine worst case rather than an optimistic one.
    let per_issue_overhead_bytes = 150;
    let sub_report_worst_case_bytes = MAX_CONTENT_ISSUES
        * (ISSUE_MESSAGE_MAX_BYTES
            + ISSUE_EVIDENCE_MAX_BYTES
            + ISSUE_SECTION_MAX_BYTES
            + per_issue_overhead_bytes);

    let wrapper = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "combined",
        "generatedAt": 1,
        "resume": { "blob": "x".repeat(sub_report_worst_case_bytes) },
        "coverLetter": { "blob": "x".repeat(sub_report_worst_case_bytes) }
    })
    .to_string();

    assert!(
        wrapper.len() < QUALITY_REPORT_MAX_BYTES,
        "a legitimate two-sub-report wrapper at the documented worst case must fit \
         under the cap — this is exactly what QUALITY_REPORT_MAX_BYTES is sized to allow"
    );
    assert_eq!(
        sanitize_quality_report(wrapper.clone(), "test"),
        wrapper,
        "a wrapper with two near-worst-case sub-reports must persist intact, not be \
         dropped to the empty sentinel"
    );
}

/// Regression for the finding this fix closes (PR #963 round 14):
/// `QUALITY_REPORT_MAX_BYTES`'s sizing arithmetic capped each `ContentIssue`
/// field at its RAW (pre-serialization) byte bound — the round-6 test above
/// exercised only that raw bound, filling the worst-case blob with plain
/// `'x'` characters, which `serde_json` never escapes. `serde_json` DOES
/// escape `"` (→ `\"`) and `\` (→ `\\`), doubling every occurrence, so a
/// legitimate quote-heavy report (quoted résumé bullets, code-snippet
/// evidence) can serialize to roughly double the raw-bound estimate. Builds
/// two sub-reports of `MAX_CONTENT_ISSUES` issues each, with
/// `message`/`evidence`/`section` filled with `"` characters at their
/// documented RAW caps (`validate::content::ISSUE_*_MAX_BYTES`) — the
/// worst-case escape density — and measures the REAL, `serde_json`-produced
/// byte length (never an assumed multiple) via `.to_string()`. Asserts the
/// escaped wrapper (a) genuinely exceeds the OLD 512 KiB cap, proving the
/// defect was reachable, and (b) still survives `sanitize_quality_report`
/// byte-for-byte under the resized cap.
#[test]
fn sanitize_quality_report_keeps_a_two_sub_report_wrapper_at_escaped_worst_case_size() {
    use crate::validate::content::{
        ISSUE_EVIDENCE_MAX_BYTES, ISSUE_MESSAGE_MAX_BYTES, ISSUE_SECTION_MAX_BYTES,
        MAX_CONTENT_ISSUES,
    };

    // One issue at its raw field caps, filled with `"` — the character that
    // costs the most once `serde_json` escapes it (`\"`, 2×).
    let issue = serde_json::json!({
        "severity": "critical",
        "code": "consistency.skill_not_demonstrated",
        "section": "\"".repeat(ISSUE_SECTION_MAX_BYTES),
        "message": "\"".repeat(ISSUE_MESSAGE_MAX_BYTES),
        "evidence": "\"".repeat(ISSUE_EVIDENCE_MAX_BYTES),
    });
    let issues: Vec<_> = std::iter::repeat_n(issue, MAX_CONTENT_ISSUES).collect();
    let sub_report = serde_json::json!({ "ok": false, "issues": issues, "metrics": {} });

    // `.to_string()` here is the real `serde_json` serialization — this is
    // the measured escaped byte count, not an assumed 2× multiple.
    let wrapper = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "combined",
        "generatedAt": 1,
        "resume": { "report": sub_report.clone(), "sourceTextHash": "x".repeat(64) },
        "coverLetter": { "report": sub_report, "sourceTextHash": "x".repeat(64) },
    })
    .to_string();

    assert!(
        wrapper.len() > 512 * 1024,
        "a quote-heavy worst-case wrapper ({} bytes) must exceed the OLD 512 KiB cap — \
         this is the exact escaping gap QUALITY_REPORT_MAX_BYTES was resized to close",
        wrapper.len()
    );
    assert!(
        wrapper.len() < QUALITY_REPORT_MAX_BYTES,
        "the resized cap ({QUALITY_REPORT_MAX_BYTES} bytes) must still allow a legitimate \
         quote-heavy worst-case wrapper ({} bytes)",
        wrapper.len()
    );
    assert_eq!(
        sanitize_quality_report(wrapper.clone(), "test"),
        wrapper,
        "an escaping-hostile worst-case wrapper must persist byte-for-byte, not be \
         dropped to the empty sentinel"
    );
}

/// End-to-end proof for the save path: `ai_generations_save` runs the
/// incoming `quality_report` through `sanitize_quality_report` (simulated
/// here exactly as the command does) BEFORE building the record it hands to
/// `save_application`. An over-cap incoming report must therefore merge as
/// content-less, so the EXISTING report survives untouched — never replaced
/// by truncated/unparseable garbage, and never silently lost with no signal
/// (the guard logs a warning; see `sanitize_quality_report`).
#[test]
fn save_application_keeps_the_existing_report_when_the_incoming_one_was_over_cap() {
    let (_dir, store) = open_store();
    let url = "https://acme.com/job/1";

    let mut first = record("g1", url);
    first.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":1,"resume":{"ok":true}}"#.into();
    store.save_application(first.clone()).unwrap();

    let oversized = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "letter",
        "generatedAt": 2,
        "coverLetter": { "blob": "x".repeat(QUALITY_REPORT_MAX_BYTES) }
    })
    .to_string();
    let mut second = content_less("g2", url);
    // Exactly what `commands::ai_generations::ai_generations_save` does to the
    // request's `quality_report` before constructing the record.
    second.quality_report = sanitize_quality_report(oversized, "test");

    store.save_application(second).unwrap();

    let stored = &store.list()[0].quality_report;
    assert_eq!(
        stored, &first.quality_report,
        "an over-cap incoming report must leave the existing report intact"
    );
    assert!(
        *stored == first.quality_report || stored.is_empty(),
        "the stored quality_report must be the existing report or the empty \
         sentinel — never unparseable truncated JSON"
    );
}

/// The scenario a raw byte clamp actually leaves unprotected: a FIRST-EVER
/// save for a job has no existing aggregate row, so `save_application` goes
/// straight to `insert()` — `merge_quality_report` never runs, and its
/// "unparseable incoming → keep existing" guard has nothing to fall back
/// onto. Without a write-boundary guard, a byte-truncated (invalid-JSON)
/// over-cap report would be stored verbatim. The command-layer guard
/// (`sanitize_quality_report`, simulated here exactly as `ai_generations_save`
/// applies it before building the record) must catch this BEFORE
/// `save_application` is ever called.
#[test]
fn save_application_inserts_the_empty_sentinel_for_a_first_save_with_an_over_cap_report() {
    let (_dir, store) = open_store();

    let oversized = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "resume",
        "generatedAt": 1,
        "resume": { "blob": "x".repeat(QUALITY_REPORT_MAX_BYTES) }
    })
    .to_string();
    let mut rec = record("g1", "https://acme.com/job/brand-new");
    // Exactly what `commands::ai_generations::ai_generations_save` does to the
    // request's `quality_report` before constructing the record.
    rec.quality_report = sanitize_quality_report(oversized, "test");

    store.save_application(rec).unwrap();

    assert_eq!(
        store.list()[0].quality_report,
        "",
        "a first-ever save (no existing row to merge onto) must store the \
         empty sentinel, never a byte-truncated invalid-JSON blob"
    );
}

/// A bundle exported before `qualityReport` existed carries no key at all;
/// `#[serde(default)]` deserializes it to `""`. A later save on that imported
/// row must not be blocked by the empty placeholder — it's simply unparseable,
/// same as any other missing report.
#[test]
fn merge_quality_report_old_bundle_import_leaves_an_empty_report_a_later_save_can_fill() {
    let (_dir, store) = open_store();
    let legacy = serde_json::json!([{
        "id": "old-1", "createdAt": 1, "candidateName": "Jane", "jobTitle": "Engineer",
        "companyName": "Acme", "resumeLanguage": "en", "jobAdLanguage": "en",
        "targetLanguage": "en", "mismatch": false, "topRequirements": [],
        "mode": "ats", "resumeText": "R", "coverLetterText": "C", "jobAd": "",
        "jobUrl": "https://acme.com/job/1", "board": "linkedin"
    }]);
    store.import(&legacy).unwrap();
    assert_eq!(store.list()[0].quality_report, "");

    let mut fresh = content_less("g2", "https://acme.com/job/1");
    fresh.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":500,"resume":{"ok":true}}"#.into();
    store.save_application(fresh).unwrap();

    assert_eq!(
        store.list()[0].quality_report,
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":500,"resume":{"ok":true}}"#
    );
}
