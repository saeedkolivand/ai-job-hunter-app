//! The report contract: `ok`, the code vocabulary, the wire shape, determinism.

use super::{support::*, *};

/// `ok` is exactly "no Criticals" — nothing else may clear or set it.
#[test]
fn ok_tracks_criticals_only() {
    let report = en_resume(EN_DUPLICATES, &en_requirements());
    assert!(
        report
            .issues
            .iter()
            .all(|i| i.severity == Severity::Warning),
        "this fixture must carry warnings only; got {:?}",
        report.issues
    );
    assert!(report.ok, "warnings alone must not clear `ok`");

    let report = en_resume(EN_FABRICATED_METRIC, &en_requirements());
    assert!(!report.ok);
}

/// Every code any fixture can produce must be registered with the severity the
/// table declares — the constructor reads the table, so this proves no check
/// reaches the "unregistered → Warning" fallback.
#[test]
fn every_emitted_code_is_registered_with_its_declared_severity() {
    let reports: Vec<ContentReport> = [
        EN_CLEAN,
        EN_FABRICATED_METRIC,
        EN_DROPPED_ROLE,
        EN_ALTERED_LINK,
        EN_DUPLICATES,
        EN_WRONG_LANGUAGE,
        EN_PROJECTS_BROKEN,
    ]
    .iter()
    .map(|generated| en_resume(generated, &en_requirements()))
    .chain([en_letter(EN_LETTER_AI_TELLS)])
    .collect();
    for report in &reports {
        for issue in &report.issues {
            let registered = CONTENT_ISSUE_CODES
                .iter()
                .find(|(c, _)| *c == issue.code)
                .unwrap_or_else(|| panic!("unregistered code emitted: {}", issue.code));
            assert_eq!(
                issue.severity, registered.1,
                "{} emitted with the wrong severity",
                issue.code
            );
        }
    }
}

/// The code vocabulary is a wire contract: the renderer keys i18n off it and a
/// stored report carries codes forever. Adding one is fine; renaming or
/// dropping one is a breaking change that must be deliberate.
#[test]
fn code_table_is_complete_and_unique() {
    let mut seen = std::collections::HashSet::new();
    for (code, _) in CONTENT_ISSUE_CODES {
        assert!(seen.insert(*code), "duplicate code in the table: {code}");
        assert!(
            code.contains('.'),
            "codes are dotted `family.check`; got {code}"
        );
    }
    assert_eq!(
        CONTENT_ISSUE_CODES.len(),
        35,
        "the code vocabulary changed — update the renderer's i18n keys too"
    );
    let criticals = CONTENT_ISSUE_CODES
        .iter()
        .filter(|(_, s)| *s == Severity::Critical)
        .count();
    assert_eq!(
        criticals, 8,
        "Criticals are deterministic factual/language/structure defects only, and every \
         one needs a RESOLUTION PATH or its run sits in needsReview forever: either a \
         REGENERATE (what `repair` does with a section-keyed finding — which is why \
         `ats.empty_section` is a Warning) or a Remove/Keep row in the review panel \
         (`report::FABRICATION_CODES`, which is how the two credential Criticals \
         clear, since `SectionKey` has no Certifications variant to regenerate)."
    );
}

/// The serialized shape of a report IS a wire contract: `ContentReportPayload`
/// in `packages/shared/src/ipc/contracts/resume.ts` is a hand-written mirror of
/// this struct, and nothing in the build compares the two. This test pins the
/// exact key set and the exact serialization of the three fields that are
/// `Option` on the Rust side.
///
/// `section`, `evidence` and `keywordCoverage` serialize as `null`, NOT omitted
/// — there is no `skip_serializing_if` on them, and the renderer's types say
/// `string | null`. If a future edit adds one, TypeScript will keep compiling
/// and every `report.metrics.keywordCoverage === null` branch will silently stop
/// matching. That is what this test is here to catch.
#[test]
fn serialized_report_matches_the_typescript_wire_mirror() {
    let report = ContentReport {
        ok: false,
        issues: vec![
            ContentIssue {
                severity: Severity::Critical,
                code: FACTUAL_UNSOURCED_METRIC,
                section: None,
                message: "m".to_string(),
                evidence: None,
            },
            ContentIssue {
                severity: Severity::Warning,
                code: ALIGNMENT_LOW_COVERAGE,
                section: Some("Experience".to_string()),
                message: "m".to_string(),
                evidence: Some("40% vs 60%".to_string()),
            },
        ],
        metrics: ContentMetrics {
            keyword_coverage: None,
            top_requirement_hits: Some(3),
            top_requirements_measured: Some(4),
            duplicate_ratio: 0.25,
            roles_source: 2,
            roles_output: 1,
        },
    };
    let value = serde_json::to_value(&report).expect("a report must serialize");

    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v
            .as_object()
            .expect("object")
            .keys()
            .map(String::from)
            .collect();
        k.sort();
        k
    };
    assert_eq!(keys(&value), ["issues", "metrics", "ok"]);
    assert_eq!(
        keys(&value["issues"][0]),
        ["code", "evidence", "message", "section", "severity"],
        "an issue's key set is the renderer's contract"
    );
    assert_eq!(
        keys(&value["metrics"]),
        [
            "duplicateRatio",
            "keywordCoverage",
            "rolesOutput",
            "rolesSource",
            "topRequirementHits",
            "topRequirementsMeasured"
        ],
        "metrics keys are camelCase on the wire"
    );

    // Severity is lowercase, matching `'critical' | 'warning'` in TS.
    assert_eq!(value["issues"][0]["severity"], "critical");
    assert_eq!(value["issues"][1]["severity"], "warning");

    // The four nullable fields are PRESENT and null, never absent.
    assert!(value["issues"][0]["section"].is_null());
    assert!(value["issues"][0]["evidence"].is_null());
    assert!(value["metrics"]["keywordCoverage"].is_null());
    assert_eq!(value["metrics"]["topRequirementHits"], 3);
    assert_eq!(value["metrics"]["topRequirementsMeasured"], 4);
    let unmeasured = serde_json::to_value(ContentMetrics {
        top_requirement_hits: None,
        top_requirements_measured: None,
        ..Default::default()
    })
    .expect("metrics must serialize");
    for field in ["topRequirementHits", "topRequirementsMeasured"] {
        assert!(
            unmeasured[field].is_null(),
            "{field}: an unmeasured value is present-and-null, matching `number | null` in TS"
        );
    }
    assert_eq!(value["issues"][1]["section"], "Experience");
    assert_eq!(value["issues"][1]["evidence"], "40% vs 60%");
    assert_eq!(value["issues"][1]["code"], ALIGNMENT_LOW_COVERAGE);
}

/// Every issue must carry evidence a user can check for themselves, or a
/// document-wide finding with a message that stands alone.
#[test]
fn every_issue_is_evidence_backed_and_advisory() {
    let report = en_resume(EN_FABRICATED_METRIC, &en_requirements());
    for issue in &report.issues {
        assert!(
            issue.evidence.is_some(),
            "{} must name what it found",
            issue.code
        );
        assert!(
            !issue.message.trim().is_empty(),
            "{} must explain itself",
            issue.code
        );
    }
}

/// Same input, same report — a validator whose output shifts between runs
/// cannot be snapshotted, cached, or trusted.
#[test]
fn validation_is_deterministic() {
    let requirements = en_requirements();
    let first = en_resume(EN_DUPLICATES, &requirements);
    for _ in 0..5 {
        assert_eq!(
            en_resume(EN_DUPLICATES, &requirements),
            first,
            "repeated runs must produce an identical report"
        );
    }
}
