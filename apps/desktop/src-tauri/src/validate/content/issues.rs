//! Issue construction and the report-size caps: the one chokepoint every check
//! builds a [`ContentIssue`] through, and the bound on how many survive.

use crate::validate::Severity;

use super::{severity_for, ContentIssue, REPORT_TRUNCATED};

/// Byte cap on [`ContentIssue::message`], enforced in [`issue`]. [`MAX_CONTENT_ISSUES`]
/// (M-3) bounds the issue *count*, not the *size* of any one issue — and
/// `ats.long_bullet` / `ats.header_in_body` / `duplicate.bullet` all quote an
/// offending span verbatim (`long_bullet` fires *because* the bullet is long,
/// so its evidence is unbounded by construction).
///
/// Sized together with [`ISSUE_EVIDENCE_MAX_BYTES`] and
/// [`ISSUE_SECTION_MAX_BYTES`], which are the OTHER two fields carrying text
/// copied out of an untrusted document. Worst case per sub-report:
/// `MAX_CONTENT_ISSUES` (200) × (400 + 400 + 120 + ~150 bytes of JSON overhead
/// for the rest of a `ContentIssue`) ≈ 214 KB raw — roughly double that once
/// JSON escaping is priced in — against `QUALITY_REPORT_MAX_BYTES` (1 MiB,
/// `commands::ai_generations::ai_generations_save`) for a wrapper that also
/// holds a second sub-report.
///
/// The point of the arithmetic is that every term in it is BOUNDED — that is
/// what keeps the save path's drop-to-sentinel branch off the table for
/// realistic content, where an issue's message is one sentence and its section
/// a two-word heading, and what keeps even a hostile document at a fixed
/// multiple of these constants instead of at the length of whatever the model
/// emitted. `section` was the term that broke it: it was copied verbatim, and
/// an ATX heading (`# …`) has no length rule anywhere in the parser, so a 1 KB
/// heading multiplied by the per-line issues underneath it made the total
/// unbounded and the claim false.
pub const ISSUE_MESSAGE_MAX_BYTES: usize = 400;

/// Byte cap on [`ContentIssue::evidence`]. See [`ISSUE_MESSAGE_MAX_BYTES`] for
/// the arithmetic this is sized against.
pub const ISSUE_EVIDENCE_MAX_BYTES: usize = 400;

/// Byte cap on [`ContentIssue::section`]. Much smaller than the other two
/// because a section LABEL is short — "PROFESSIONAL EXPERIENCE" is 23 bytes and
/// the longest heading any template renders is well inside this — while the
/// value itself is untrusted: `section` is a heading line copied out of the
/// generated document, and an ATX heading (`# …`) has no length rule anywhere
/// in the parser. Unclamped, one 1 KB heading multiplied by the per-line issues
/// under it (`ats.long_bullet`, `duplicate.bullet`, …) put the serialized
/// report back over the save path's clamp that [`MAX_CONTENT_ISSUES`] exists to
/// keep it under. Folded into that arithmetic in [`ISSUE_MESSAGE_MAX_BYTES`].
pub const ISSUE_SECTION_MAX_BYTES: usize = 120;

/// `…` truncation marker appended by [`clamp_issue_text`] when it cuts
/// anything, so a clamped span reads as visibly cut rather than as the whole
/// span. Its bytes come out of the budget (not added on top), so the result
/// never exceeds `max` — the arithmetic on [`ISSUE_MESSAGE_MAX_BYTES`] stays
/// exact.
const TRUNCATION_MARKER: &str = "…";

/// Clamp `s` to at most `max` bytes, UTF-8 char-boundary safe (delegates to
/// [`crate::applications::clamp_to_bytes`] rather than forking a second
/// truncation routine), reserving room for [`TRUNCATION_MARKER`] so a cut
/// result is still at most `max` bytes, never `max` bytes plus the marker.
fn clamp_issue_text(s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let budget = max.saturating_sub(TRUNCATION_MARKER.len());
    let mut clamped = crate::applications::clamp_to_bytes(s, budget);
    clamped.push_str(TRUNCATION_MARKER);
    clamped
}

/// Build an issue, reading its severity from [`CONTENT_ISSUE_CODES`].
///
/// All THREE untrusted fields — `message`, `evidence` and `section` — are
/// clamped here, at the one chokepoint every call site routes through, so no
/// validator has to remember to bound its own span. `section` is untrusted for
/// the same reason the other two are: it is a heading line copied out of the
/// generated document.
pub(crate) fn issue(
    code: &'static str,
    section: Option<&str>,
    message: impl Into<String>,
    evidence: Option<String>,
) -> ContentIssue {
    ContentIssue {
        severity: severity_for(code),
        code,
        section: section.map(|s| clamp_issue_text(s.to_string(), ISSUE_SECTION_MAX_BYTES)),
        message: clamp_issue_text(message.into(), ISSUE_MESSAGE_MAX_BYTES),
        evidence: evidence.map(|e| clamp_issue_text(e, ISSUE_EVIDENCE_MAX_BYTES)),
    }
}

// ── Thresholds shared across validators ─────────────────────────────────────

/// Hard cap on [`ContentReport::issues`]' length. M-3: without this, a
/// pathological/hostile "generated" document (thousands of forged roles,
/// duplicate bullets, etc.) can grow the serialized report without bound —
/// past the save path's `QUALITY_REPORT_MAX_BYTES` (1 MiB,
/// `commands::ai_generations::ai_generations_save`) byte clamp. That clamp
/// truncates mid-JSON, the stored blob becomes unparseable, and
/// `ai_generations::merge_quality_report` then silently falls back to
/// keeping the OLD stored report — a fresh report just vanishes with no
/// error anywhere. Capping the issue list here, at the source, keeps the
/// serialized report comfortably under that clamp so the truncate-then-fail
/// path is unreachable. Mirrors the same count-cap discipline the now-deleted
/// `agent::tools_quality::MAX_ISSUES` used, sized higher (200 vs. 20) because this report is
/// the FULL quality-report panel's data, not a token-budgeted tool summary.
pub const MAX_CONTENT_ISSUES: usize = 200;

/// Cap an issue list at [`MAX_CONTENT_ISSUES`], criticals first, with ONE
/// visible truncation marker.
///
/// M-3: without this, a pathological/hostile "generated" document (thousands of
/// forged roles, duplicate bullets) grows the serialized report past the save
/// path's `QUALITY_REPORT_MAX_BYTES` clamp, which truncates mid-JSON and makes
/// `merge_quality_report` silently keep the OLD stored report. Criticals sort
/// first (a stable sort, so their relative order survives) so a warning flood
/// can never push a real Critical out of the visible list — only Warnings are
/// ever cut, and the trailing `REPORT_TRUNCATED` marker says so instead of a
/// silent drop.
///
/// **A FUNCTION, and re-runnable, because the report is written twice.**
/// `validate_content` caps what it found; `stages::judge` then merges up to
/// [`MAX_JUDGE_ITEMS`](crate::pipeline::resume::stages::MAX_JUDGE_ITEMS) more
/// into the SAME list at max depth, which put the list back over the bound the
/// `QUALITY_REPORT_MAX_BYTES` derivation rests on — and did it by appending
/// Warnings, exactly the class the criticals-first sort exists to cut first. An
/// existing marker is ABSORBED (its own dropped count carried into the new one)
/// rather than left beside a second one, so calling this again is safe and the
/// count stays truthful.
pub(crate) fn cap_issues(issues: &mut Vec<ContentIssue>) {
    let mut dropped = 0usize;
    issues.retain(|candidate| {
        if candidate.code != REPORT_TRUNCATED {
            return true;
        }
        // `unwrap_or(1)`, not `0`: a marker whose count cannot be read still
        // means "issues were dropped", and defaulting to zero would ABSORB the
        // marker and then decline to re-emit it — turning an unreadable count
        // into a report that silently claims nothing was truncated.
        dropped += candidate
            .evidence
            .as_deref()
            .and_then(|count| count.parse::<usize>().ok())
            .unwrap_or(1);
        false
    });
    if issues.len() > MAX_CONTENT_ISSUES {
        dropped += issues.len() - MAX_CONTENT_ISSUES;
        issues.sort_by_key(|i| i.severity != Severity::Critical);
        issues.truncate(MAX_CONTENT_ISSUES);
    }
    if dropped == 0 {
        return;
    }
    issues.push(issue(
        REPORT_TRUNCATED,
        None,
        format!(
            "{dropped} more issue{} found but not shown here — this document has an \
             unusually large number of findings.",
            if dropped == 1 { "" } else { "s" }
        ),
        Some(dropped.to_string()),
    ));
}
