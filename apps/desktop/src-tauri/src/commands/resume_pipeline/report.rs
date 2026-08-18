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
//!   the other document's next save. Each entry carries the containing `line`
//!   (see [`containing_line`]) so a "Remove" can anchor on a whole line rather
//!   than substring-matching a bare token.

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
/// The two credential Criticals are here because a Critical that is NOT listed
/// keeps its run in `needsReview` forever ([`still_needs_review`]), and both
/// name a span the user can look at and decide about.
///
/// Their repair stories DIFFER, and the difference matters:
/// `factual.unsourced_certification` has no section the loop could regenerate
/// (`SectionKey` has no Certifications variant), so the panel is its only
/// resolution path; `factual.inflated_experience` DOES route — its section is
/// the summary — so a false one would spend real provider calls rewriting a
/// correct summary against "offending text: 15 years", and could pressure the
/// model into understating a true tenure. That is the sharper reason its
/// false-positive rate had to be measured at zero before it shipped Critical,
/// and it is why listing it here is a resolution path rather than a workaround.
///
/// `factual.unsourced_institution` is deliberately ABSENT. It is a Warning, and
/// listing a Warning here would park the run until the user decided it — which
/// is exactly the "advisory" claim its registration makes being false. The
/// precedent that says otherwise (`factual.unsourced_term`, also a Warning) is
/// left alone rather than copied.
const FABRICATION_CODES: &[&str] = &[
    crate::validate::content::FACTUAL_UNSOURCED_METRIC,
    crate::validate::content::FACTUAL_UNSUPPORTED_DATE,
    crate::validate::content::FACTUAL_UNSOURCED_TERM,
    crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK,
    crate::validate::content::FACTUAL_INFLATED_EXPERIENCE,
    crate::validate::content::FACTUAL_UNSOURCED_CERTIFICATION,
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

/// Cap on the `line` an entry carries.
///
/// A flagged claim lives in a bullet, and a bullet is prose — a "line" past
/// this length is a document with no newlines in it at all (a paste artifact),
/// not something a user reviews line by line. The field is then OMITTED rather
/// than truncated: a Remove anchors on an EXACT line match, so a prefix would
/// match nothing anyway, and carrying the whole blob would multiply it by every
/// entry in the list inside a column that is already persisted per posting.
pub(super) const MAX_LINE_CHARS: usize = 1_000;

/// The full (trimmed) text of the line of `text` that contains `span` — when
/// exactly ONE line does.
///
/// This is the anchor a "Remove" verdict is applied against, and it exists
/// because `evidence` alone is not one: a validator span is routinely a bare
/// token (`"250"`, one keyword), so a renderer deleting "every line containing
/// the evidence" deletes whatever else happens to quote it — the contact header
/// among them, which is how a Remove came to erase a phone number. Located HERE
/// rather than in the renderer because this is the only layer that still holds
/// the exact text the report was produced over (`slot`'s `text`); by the time
/// the panel reads the entry the document may already have moved.
///
/// `None` — the field is then absent — wherever no honest anchor exists:
///
/// * the span is not in the document (an entry re-issued over text that was
///   already edited, or a validator span assembled rather than quoted). A
///   fabricated anchor would be applied to the wrong line;
/// * the span occurs on MORE THAN ONE line. Two of the reviewable codes
///   routinely emit non-unique spans (`factual.unsupported_date` is a bare
///   year, `factual.unsourced_metric` a bare figure), and picking the first
///   occurrence in document order anchors on whichever line happens to come
///   first — an education year instead of the flagged job entry. That is the
///   substring failure one level up: deterministic, but about the wrong line.
///   A span naming several lines has no honest anchor, and the renderer's
///   `removeEvidenceLines` already refuses safely on a missing one;
/// * the span straddles a newline — no single line contains it, so no
///   single-line anchor can stand for the whole of it;
/// * the containing line is blank or longer than [`MAX_LINE_CHARS`].
///
/// Still deterministic — uniqueness is a property of the text, so the same
/// report over the same text produces the same entry on every re-issue.
fn containing_line(text: &str, span: &str) -> Option<String> {
    let span = span.trim();
    if span.is_empty() {
        return None;
    }
    let mut containing = text.lines().filter(|line| line.contains(span));
    let line = containing.next()?;
    if containing.next().is_some() {
        return None;
    }
    let line = line.trim();
    (!line.is_empty() && line.chars().count() <= MAX_LINE_CHARS).then(|| line.to_string())
}

/// One flagged span awaiting (or carrying) the user's verdict, with the
/// document line it sits on ([`containing_line`]).
fn fabrications(report: &ContentReport, text: &str) -> Vec<Value> {
    report
        .issues
        .iter()
        .enumerate()
        .filter(|(_, issue)| FABRICATION_CODES.contains(&issue.code))
        .filter_map(|(index, issue)| {
            // No span, nothing to review. A finding the panel cannot show the
            // evidence for is one the user cannot make a decision about.
            let evidence = issue.evidence.clone()?;
            // `factual.altered_project_link` is emitted from two arms and only
            // one is reviewable: a link the model INVENTED sits in the
            // generated text, so Keep/Remove is a real question about a span
            // the user can look at. The other arm — a SOURCE link missing or
            // altered in the output — names an ABSENCE, exactly like
            // `factual.dropped_role`: its evidence is the source URL, which by
            // definition is not in this document, so a review row would ask a
            // question with no correct answer (and render as "you may have
            // edited this away", which is not what happened). It stays a
            // Critical in the report, which is what keeps the run at
            // `needsReview`. Scoped to this ONE code rather than a general
            // presence gate: `factual.unsourced_term`'s evidence is a
            // NORMALIZED token ("kubernetes" for a document that says
            // "Kubernetes"), so gating every code on `contains` would silently
            // drop genuine entries.
            if issue.code == crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK
                && !text.contains(evidence.trim())
            {
                return None;
            }
            let line = containing_line(text, &evidence);
            let mut entry = Map::new();
            entry.insert(
                "issueKey".to_string(),
                json!(format!("{}#{index}", issue.code)),
            );
            entry.insert("code".to_string(), json!(issue.code));
            entry.insert("evidence".to_string(), json!(evidence));
            if let Some(line) = line {
                entry.insert("line".to_string(), json!(line));
            }
            Some(Value::Object(entry))
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
    let flagged = fabrications(report, text);
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

/// Whether one entry is genuinely settled, against the document as it stands
/// NOW — the renderer's `isFabricationResolved`, mirrored.
///
/// A decision alone is not enough. `keep` settles the entry outright (nothing
/// has to change). `remove` settles it only once the flagged span is actually
/// ABSENT from the current text: the verdict is a record of INTENT, the
/// document is the record of FACT, and they agree only when the span is gone.
/// Counting a recorded-but-unapplied Remove as resolved is what let
/// `resolveFabrication` flip a run to `completed` while the review panel —
/// which applies exactly this rule — still showed the same entry as pending:
/// the two sides of one run disagreeing about whether it was finished.
///
/// An unrecognized decision token reads as undecided (the renderer's
/// `parseDecision` rule), never as a verdict the user never gave. An entry
/// with a blank span reads as absent rather than as a match against
/// everything.
fn entry_resolved(entry: &Value, document_text: &str) -> bool {
    match entry.get("decision").and_then(Value::as_str) {
        Some("keep") => true,
        Some("remove") => {
            let span = entry
                .get("evidence")
                .and_then(Value::as_str)
                .map_or("", str::trim);
            span.is_empty() || !document_text.contains(span)
        }
        _ => false,
    }
}

/// How many findings in a persisted wrapper are still unfinished, across both
/// documents — undecided, or carrying a Remove the document has not caught up
/// with ([`entry_resolved`]).
///
/// Takes the CURRENT text of each document because "resolved" is a property of
/// the pair (verdict, document), not of the wrapper alone — the same rule the
/// renderer's `unresolvedCount` applies, so the badge and the run row can
/// never disagree about whether a run is finished.
///
/// The COUNT rather than the bool because the terminal notification says how
/// much work is left ("2 flagged claims need a Keep or Remove decision"), and
/// deriving that number anywhere else would be a second definition of
/// "unresolved" to keep in step with this one.
pub fn unresolved_count(wrapper: &str, resume_text: &str, cover_letter_text: &str) -> usize {
    let Ok(parsed) = serde_json::from_str::<Value>(wrapper) else {
        return 0;
    };
    [("resume", resume_text), ("coverLetter", cover_letter_text)]
        .iter()
        .filter_map(|(document, text)| {
            let entries = parsed
                .get(*document)
                .and_then(|slot| slot.get("fabrications"))
                .and_then(Value::as_array)?;
            Some((entries, *text))
        })
        .flat_map(|(entries, text)| entries.iter().map(move |entry| (entry, text)))
        .filter(|(entry, text)| !entry_resolved(entry, text))
        .count()
}

/// Whether a persisted wrapper still has an unfinished finding.
///
/// This is what keeps a run at `needsReview`: nothing is ever removed silently,
/// so a flagged bullet without a verdict — or with a Remove the document has
/// not caught up with — must never be presented as clean. Delegates to
/// [`unresolved_count`] so the gate and the number the user is shown can never
/// disagree.
pub fn has_unresolved(wrapper: &str, resume_text: &str, cover_letter_text: &str) -> bool {
    unresolved_count(wrapper, resume_text, cover_letter_text) > 0
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
/// * an unfinished fabrication — undecided, or a Remove whose span is still in
///   the document ([`entry_resolved`]). Nothing is removed silently, so a
///   flagged bullet without a verdict the document agrees with is unfinished
///   business;
/// * a Critical the review CANNOT resolve. `factual.dropped_role` is the case:
///   it names an absence, so it is deliberately not listed in the Remove/Keep
///   panel ([`FABRICATION_CODES`]) — and a run that flipped to `completed`
///   because every *reviewable* finding was decided would be presenting a
///   résumé that silently lost an employer as clean.
///
/// An unparseable or absent wrapper is NOT blocking: "no report" is the
/// fast-path/no-validation state, and inventing a review requirement for it
/// would strand every run that predates the column.
pub fn still_needs_review(wrapper: &str, resume_text: &str, cover_letter_text: &str) -> bool {
    if has_unresolved(wrapper, resume_text, cover_letter_text) {
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
