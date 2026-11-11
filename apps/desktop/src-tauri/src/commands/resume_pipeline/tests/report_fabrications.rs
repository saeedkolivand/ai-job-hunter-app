use super::super::report;
use super::support::{
    fabrication_report, only_fabrication, report_for, CLEAN_SOURCE, FABRICATING_DRAFT,
};
use serde_json::json;

/// **Every entry carries the whole LINE its evidence sits on** — the anchor a
/// "Remove" is applied against.
///
/// `evidence` is NOT that anchor and never was: the validator's span is
/// routinely a bare token (`"47%"`, one keyword), so a renderer deleting "every
/// line containing the evidence" deletes whatever else quotes it — which is how
/// a Remove came to erase the contact header. The line is located HERE, at
/// report-build time, because this is the last layer holding the exact text the
/// report was produced over.
///
/// This is also the SERIALIZED-SHAPE pin the renderer's contract rests on:
/// dropping the field (mutation: delete the `entry.insert("line", …)` branch)
/// fails the `line` lookup below, applied and reverted.
#[test]
fn every_fabrication_entry_anchors_on_the_document_line_it_was_found_on() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).expect("valid JSON");
    let flagged = parsed["resume"]["fabrications"]
        .as_array()
        .expect("the fabricated metric is listed for review");
    assert!(!flagged.is_empty());

    for entry in flagged {
        let evidence = entry["evidence"].as_str().expect("a span");
        let line = entry["line"]
            .as_str()
            .unwrap_or_else(|| panic!("{evidence:?} was quoted from the draft, so it HAS a line"));
        assert!(
            line.contains(evidence.trim()),
            "the anchor must contain the span it was located from: {line:?} vs {evidence:?}"
        );
        // Trimmed, and a REAL line of the document — the renderer matches a
        // whole trimmed line, so a fragment or an untrimmed copy anchors on
        // nothing.
        assert_eq!(line, line.trim());
        assert!(
            FABRICATING_DRAFT.lines().any(|l| l.trim() == line),
            "{line:?} is not a line of the validated document"
        );
    }
    assert!(
        flagged.iter().any(|entry| entry["line"]
            == json!("A payments engineer who cut costs by 47% across 12 teams.")),
        "the fabricated metric's own bullet is the anchor: {flagged:?}"
    );
}

/// **A span occurring on several lines carries NO anchor at all.**
///
/// Two of the reviewable codes routinely emit non-unique spans — a
/// `factual.unsupported_date` is a bare year, a `factual.unsourced_metric` a
/// bare figure — and an anchor picked by document order names whichever line
/// happens to come FIRST: a year sitting in an education line would anchor the
/// verdict for a flagged job entry, and the Remove would delete the education
/// line. Determinism is not correctness; refusing is, and the renderer's
/// `removeEvidenceLines` refuses safely on a missing anchor.
///
/// Mutation check: anchor on the first occurrence (the old behaviour) and the
/// `is_none` assertions fail with the education line as the anchor.
#[test]
fn a_span_on_several_lines_carries_no_anchor() {
    // The review's own scenario: the flagged year also sits in an education
    // line that comes FIRST in document order.
    let text = "EDUCATION\nB.Sc. Computer Science, 2019\n\nWORK EXPERIENCE\nEngineer | Acme | 2019 - Present\n";
    let report = fabrication_report("2019");
    let wrapper = report::build("quality", 1, Some((&report, text)), None);
    let entry = only_fabrication(&wrapper);
    assert!(
        entry.get("line").is_none(),
        "a non-unique span has no honest anchor: {entry}"
    );
    // Still decidable — the entry survives, only the automatic apply is off.
    let key = entry["issueKey"].as_str().expect("a key").to_string();
    assert!(report::record_decision(&wrapper, &key, "remove").is_some());

    // Re-issued from the same report + text: byte-identical — uniqueness is a
    // property of the text, so the refusal is as deterministic as the anchor.
    assert_eq!(
        report::build("quality", 1, Some((&report, text)), None),
        wrapper
    );

    // A bare-figure metric, same shape: "250" in the flagged bullet AND in a
    // second line. Neither line may be guessed at.
    let text = "PROFESSIONAL SUMMARY\nCut costs by 250 hours a month.\n\nWORK EXPERIENCE\n- Saved 250 hours again.\n";
    let wrapper = report::build("quality", 1, Some((&fabrication_report("250"), text)), None);
    assert!(only_fabrication(&wrapper).get("line").is_none());

    // …and the SAME span twice on ONE line still anchors: the line is unique,
    // which is the property the removal needs.
    let text = "PROFESSIONAL SUMMARY\nCut 250 costs by 250 hours.\n";
    let wrapper = report::build("quality", 1, Some((&fabrication_report("250"), text)), None);
    assert_eq!(
        only_fabrication(&wrapper)["line"],
        json!("Cut 250 costs by 250 hours.")
    );
}

/// **No honest anchor → NO key**, rather than a guessed one.
///
/// Two cases, and both are real: a span the document no longer contains (the
/// entry was re-issued over text the user already edited), and a line so long it
/// is not a bullet anyone reviews. The entry itself survives in both — it still
/// has to be decidable, or the run is stranded at `needsReview` forever — and
/// the renderer is told "cannot apply automatically" by the field's absence.
///
/// Mutation check: fall back to `text` (or to the evidence) instead of `None`
/// and the `is_none` assertions fail.
#[test]
fn an_entry_with_no_locatable_line_omits_the_anchor_but_stays_decidable() {
    // (1) The span is not in the document at all.
    let wrapper = report::build(
        "quality",
        1,
        Some((
            &fabrication_report("47%"),
            "PROFESSIONAL SUMMARY\nA payments engineer.\n",
        )),
        None,
    );
    let entry = only_fabrication(&wrapper);
    assert!(
        entry.get("line").is_none(),
        "an unlocatable span must carry no anchor: {entry}"
    );
    let key = entry["issueKey"].as_str().expect("a key");
    assert!(
        report::record_decision(&wrapper, key, "remove").is_some(),
        "the finding is still decidable — otherwise the run never leaves review"
    );

    // (2) The containing line is past the cap.
    let long_line = format!(
        "{} 47% {}",
        "x".repeat(report::MAX_LINE_CHARS),
        "y".repeat(50)
    );
    let text = format!("PROFESSIONAL SUMMARY\n{long_line}\n");
    let over_cap = report::build(
        "quality",
        1,
        Some((&fabrication_report("47%"), &text)),
        None,
    );
    assert!(
        only_fabrication(&over_cap).get("line").is_none(),
        "a line past MAX_LINE_CHARS is a paste artifact, not a reviewable bullet"
    );

    // …and one character under the cap still anchors, so the guard is a cap and
    // not an off-switch.
    let at_cap = "z".repeat(report::MAX_LINE_CHARS - 4) + " 47%";
    let text = format!("PROFESSIONAL SUMMARY\n{at_cap}\n");
    let within = report::build(
        "quality",
        1,
        Some((&fabrication_report("47%"), &text)),
        None,
    );
    assert_eq!(only_fabrication(&within)["line"], json!(at_cap));
}

fn critical(code: &'static str, evidence: &str) -> crate::validate::content::ContentReport {
    use crate::validate::content::{ContentIssue, ContentMetrics, ContentReport};
    ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: crate::validate::Severity::Critical,
            code,
            section: None,
            message: "m".to_string(),
            evidence: Some(evidence.to_string()),
        }],
        metrics: ContentMetrics::default(),
    }
}

/// **A Remove must never delete more than the finding.** An invented location
/// sits on the role's HEADER line, and a letter's term finding sits in a whole
/// paragraph, so neither carries an anchor: the renderer then records the
/// verdict and asks the user to edit the text, and only Keep resolves it.
///
/// Mutation check: drop `allows_line_removal`'s filter and the anchors below
/// reappear.
#[test]
fn a_location_or_letter_term_finding_carries_no_line_to_delete() {
    use crate::validate::content::{FACTUAL_CONFLATED_ROLE_TERM, FACTUAL_INVENTED_LOCATION};
    let resume =
        "Jane Doe\n\nEXPERIENCE\n\nEngineer | Acme | Berlin | 2020 - 2024\n- Shipped the ledger\n";
    let location = critical(FACTUAL_INVENTED_LOCATION, "Berlin");
    let wrapper = report::build("quality", 1, Some((&location, resume)), None);
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
    let entry = &parsed["resume"]["fabrications"][0];
    assert_eq!(entry["evidence"], json!("Berlin"));
    assert!(entry.get("line").is_none(), "{entry}");

    let letter = "Dear Hiring Manager,\n\nAt Acme I ran Redis for the billing team.\n";
    let term = critical(FACTUAL_CONFLATED_ROLE_TERM, "Redis");
    let wrapper = report::build("quality", 1, None, Some((&term, letter)));
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
    assert!(parsed["coverLetter"]["fabrications"][0]
        .get("line")
        .is_none());

    // The same code in a RÉSUMÉ quotes a whole bullet, which is safe to remove.
    let bullet = "Jane Doe\n\nEXPERIENCE\n\n- Ran Redis for the billing team\n";
    let wrapper = report::build(
        "quality",
        1,
        Some((
            &critical(
                FACTUAL_CONFLATED_ROLE_TERM,
                "- Ran Redis for the billing team",
            ),
            bullet,
        )),
        None,
    );
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
    assert!(parsed["resume"]["fabrications"][0].get("line").is_some());
}

/// **An invented location is reconciled against ITS occurrence.** "Berlin" is
/// invented on Globex's header but true on Acme's: no `line` (no header
/// deletion), a `context` naming the Globex header, and a recorded Remove that
/// settles once that header is edited even though "Berlin" survives on Acme's.
///
/// Mutations (each applied and reverted): drop the `context` insert; make
/// `entry_resolved` ignore `context`; make `context_line` skip the identity
/// narrowing. Each fails an assertion below.
#[test]
fn an_invented_location_settles_on_its_own_occurrence_not_the_word_documentwide() {
    use crate::validate::content::{
        ContentIssue, ContentMetrics, ContentReport, FACTUAL_INVENTED_LOCATION,
    };
    let globex = "Engineer | Globex Logistics | Berlin | 2018 - 2021";
    let resume = format!(
        "Jane Doe\n\nEXPERIENCE\n\nDeveloper | Acme | Berlin | 2015 - 2018\n- Built it\n\n{globex}\n- Shipped\n"
    );
    let report = ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: crate::validate::Severity::Critical,
            code: FACTUAL_INVENTED_LOCATION,
            section: None,
            message: "Your source résumé gives no location \"Berlin\" for \"Globex Logistics\". Remove it.".to_string(),
            evidence: Some("Berlin".to_string()),
        }],
        metrics: ContentMetrics::default(),
    };
    let wrapper = report::build("quality", 1, Some((&report, &resume)), None);
    let entry = only_fabrication(&wrapper);
    assert!(entry.get("line").is_none(), "{entry}");
    assert_eq!(entry["context"], json!(globex));

    let decided = report::record_decision(&wrapper, "factual.invented_location#0", "remove")
        .expect("entry matched");
    // Recorded but not edited: still pending.
    assert_eq!(report::unresolved_count(&decided, &resume, ""), 1);
    // Header edited, Acme still says Berlin: settled.
    let edited = resume.replace(globex, "Engineer | Globex Logistics | 2018 - 2021");
    assert!(edited.contains("Berlin"));
    assert_eq!(report::unresolved_count(&decided, &edited, ""), 0);
}
