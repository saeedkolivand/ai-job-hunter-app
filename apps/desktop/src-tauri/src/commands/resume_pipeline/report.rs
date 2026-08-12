//! The persisted quality-report wrapper, Rust side.
//!
//! `AiGenerationRecord.quality_report` holds a renderer-shaped JSON wrapper
//! (`{schemaVersion, pipeline, generatedAt, resume?, coverLetter?}`) that the
//! store treats as opaque and merges PER TOP-LEVEL KEY. The pipeline writes the
//! same shape, so a quality run and a fast-path save land in one column without
//! either clobbering the other's document.
//!
//! Two additions the renderer must tolerate, both documented in the IPC
//! contract:
//!
//! * `pipeline` is the DEPTH (`"quality"`), not always `"fast"`. Lying about it
//!   would be the cheaper diff and would mislabel every panel.
//! * a slot may carry `fabrications` — the surviving findings awaiting a
//!   per-bullet verdict. They live INSIDE the document's slot for the same
//!   reason `sourceTextHash` does: the merge overlays whole top-level keys, so
//!   anything belonging to one document that sits beside it gets orphaned by
//!   the other document's next save.

use serde_json::{json, Map, Value};

use crate::validate::content::ContentReport;
use crate::validate::Severity;

/// The codes whose findings go to the terminal per-bullet review.
///
/// The `factual.*` family MINUS `factual.dropped_role`: every code here names a
/// SPAN the user can look at and decide about, while a dropped role names an
/// ABSENCE — "keep" and "remove" are both meaningless for it, and listing it in
/// a Remove/Keep panel would ask a question with no correct answer. It still
/// appears in the report as a Critical.
const FABRICATION_CODES: &[&str] = &[
    crate::validate::content::FACTUAL_UNSOURCED_METRIC,
    crate::validate::content::FACTUAL_UNSUPPORTED_DATE,
    crate::validate::content::FACTUAL_UNSOURCED_TERM,
    crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK,
];

/// djb2, byte-for-byte the renderer's `hashText`.
///
/// Both halves of the algorithm have to match or every persisted report reads
/// as stale the moment it is reopened:
///
/// * **UTF-16 code units**, because JS `charCodeAt` yields those — a résumé
///   with an em dash or an emoji hashes differently over bytes or over
///   `char`s.
/// * **32-bit wrapping**, because JS `^` applies `ToInt32` to both operands,
///   which truncates the `× 33` to 32 bits exactly as `wrapping_mul` does. The
///   final `>>> 0` is the `as u32`.
pub fn hash_text(text: &str) -> u32 {
    let mut hash: i32 = 5381;
    for unit in text.encode_utf16() {
        hash = hash.wrapping_mul(33) ^ i32::from(unit);
    }
    hash as u32
}

/// One flagged span awaiting (or carrying) the user's verdict.
fn fabrications(report: &ContentReport) -> Vec<Value> {
    report
        .issues
        .iter()
        .enumerate()
        .filter(|(_, issue)| FABRICATION_CODES.contains(&issue.code))
        .filter_map(|(index, issue)| {
            // No span, nothing to review. A finding the panel cannot show the
            // evidence for is one the user cannot make a decision about.
            let evidence = issue.evidence.clone()?;
            Some(json!({
                "issueKey": format!("{}#{index}", issue.code),
                "code": issue.code,
                "evidence": evidence,
            }))
        })
        .collect()
}

/// One document's slot.
fn slot(report: &ContentReport, text: &str) -> Value {
    let mut slot = Map::new();
    slot.insert(
        "report".to_string(),
        serde_json::to_value(report).unwrap_or_else(|_| json!({})),
    );
    slot.insert("sourceTextHash".to_string(), json!(hash_text(text)));
    let flagged = fabrications(report);
    // A document with nothing flagged carries NO key, rather than an empty
    // array: the renderer's "does this run still need review?" test is the
    // presence of an undecided entry, and an empty array is a shape it would
    // have to special-case.
    if !flagged.is_empty() {
        slot.insert("fabrications".to_string(), Value::Array(flagged));
    }
    Value::Object(slot)
}

/// Build the wrapper this run should write.
///
/// A document this run did not validate contributes NO key at all — the merge
/// overlays whatever keys the wrapper carries, so an `undefined`-ish key would
/// wipe the OTHER document's stored slot.
pub fn build(
    depth: &str,
    generated_at: u64,
    resume: Option<(&ContentReport, &str)>,
    cover_letter: Option<(&ContentReport, &str)>,
) -> String {
    let mut wrapper = Map::new();
    wrapper.insert("schemaVersion".to_string(), json!(2));
    wrapper.insert("pipeline".to_string(), json!(depth));
    wrapper.insert("generatedAt".to_string(), json!(generated_at));
    if let Some((report, text)) = resume {
        wrapper.insert("resume".to_string(), slot(report, text));
    }
    if let Some((report, text)) = cover_letter {
        wrapper.insert("coverLetter".to_string(), slot(report, text));
    }
    serde_json::to_string(&Value::Object(wrapper)).unwrap_or_default()
}

/// Record one Remove/Keep verdict into an already-persisted wrapper.
///
/// Returns the updated wrapper, or `None` when nothing matched — an unknown
/// `issueKey`, a report that no longer carries that finding (it was repaired
/// away by a later run), or an unparseable blob. `None` is a no-op at the
/// command, never an error: a decision about a finding that no longer exists is
/// stale, not wrong.
///
/// **Deliberately text-independent.** The stamp is matched on `issueKey` inside
/// the PERSISTED wrapper and never reads a document, so a verdict still lands
/// after the user has edited the résumé out from under the report — which is the
/// normal case once a Remove is applied (the editor's save goes through
/// `AiGenerationStore::update_texts`, which never touches `quality_report`). See
/// the module doc of [`super`] for the settled divergence semantics: what goes
/// stale is [`slot`]'s `sourceTextHash`, and that is display state, not a
/// blocker.
pub fn record_decision(wrapper: &str, issue_key: &str, decision: &str) -> Option<String> {
    let mut parsed: Value = serde_json::from_str(wrapper).ok()?;
    let mut matched = false;
    for document in ["resume", "coverLetter"] {
        let Some(entries) = parsed
            .get_mut(document)
            .and_then(|slot| slot.get_mut("fabrications"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for entry in entries.iter_mut() {
            if entry.get("issueKey").and_then(Value::as_str) == Some(issue_key) {
                if let Some(object) = entry.as_object_mut() {
                    object.insert("decision".to_string(), json!(decision));
                    matched = true;
                }
            }
        }
    }
    matched.then(|| serde_json::to_string(&parsed).unwrap_or_default())
}

/// How many findings in a persisted wrapper nobody has decided about yet,
/// across both documents.
///
/// The COUNT rather than the bool because the terminal notification says how
/// much work is left ("2 flagged claims need a Keep or Remove decision"), and
/// deriving that number anywhere else would be a second definition of
/// "undecided" to keep in step with this one.
pub fn unresolved_count(wrapper: &str) -> usize {
    let Ok(parsed) = serde_json::from_str::<Value>(wrapper) else {
        return 0;
    };
    ["resume", "coverLetter"]
        .iter()
        .filter_map(|document| {
            parsed
                .get(document)
                .and_then(|slot| slot.get("fabrications"))
                .and_then(Value::as_array)
        })
        .flatten()
        .filter(|entry| entry.get("decision").is_none())
        .count()
}

/// Whether a persisted wrapper still has a finding nobody has decided about.
///
/// This is what keeps a run at `needsReview`: nothing is ever removed silently,
/// so a run with an undecided fabrication must never be presented as clean.
/// Delegates to [`unresolved_count`] so the gate and the number the user is
/// shown can never disagree.
pub fn has_unresolved(wrapper: &str) -> bool {
    unresolved_count(wrapper) > 0
}

/// Whether a report blocks: it carries at least one Critical.
pub fn has_criticals(report: &ContentReport) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.severity == Severity::Critical)
}

/// Whether a PERSISTED wrapper still keeps its run in `needsReview`.
///
/// Two independent reasons, and both have to clear:
///
/// * an undecided fabrication — nothing is removed silently, so a flagged
///   bullet with no verdict is unfinished business;
/// * a Critical the review CANNOT resolve. `factual.dropped_role` is the case:
///   it names an absence, so it is deliberately not listed in the Remove/Keep
///   panel ([`FABRICATION_CODES`]) — and a run that flipped to `completed`
///   because every *reviewable* finding was decided would be presenting a
///   résumé that silently lost an employer as clean.
///
/// An unparseable or absent wrapper is NOT blocking: "no report" is the
/// fast-path/no-validation state, and inventing a review requirement for it
/// would strand every run that predates the column.
pub fn still_needs_review(wrapper: &str) -> bool {
    if has_unresolved(wrapper) {
        return true;
    }
    let Ok(parsed) = serde_json::from_str::<Value>(wrapper) else {
        return false;
    };
    ["resume", "coverLetter"]
        .iter()
        .any(|document| slot_has_unresolvable_critical(parsed.get(document)))
}

/// Whether one document's slot carries a Critical that the per-bullet review
/// cannot clear — i.e. a Critical whose `issueKey` is not a DECIDED fabrication.
fn slot_has_unresolvable_critical(slot: Option<&Value>) -> bool {
    let Some(slot) = slot else { return false };
    let decided: Vec<&str> = slot
        .get("fabrications")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| entry.get("decision").is_some())
                .filter_map(|entry| entry.get("issueKey").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    slot.get("report")
        .and_then(|report| report.get("issues"))
        .and_then(Value::as_array)
        .is_some_and(|issues| {
            issues.iter().enumerate().any(|(index, issue)| {
                if issue.get("severity").and_then(Value::as_str) != Some("critical") {
                    return false;
                }
                let Some(code) = issue.get("code").and_then(Value::as_str) else {
                    return true;
                };
                !decided.contains(&format!("{code}#{index}").as_str())
            })
        })
}
