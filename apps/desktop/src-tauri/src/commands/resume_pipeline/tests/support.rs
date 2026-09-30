use crate::pipeline::StageInfo;
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentMetrics, ContentReport, DocKind,
};

/// A [`StageInfo`] whose only interesting field is whether the stage costs a
/// provider call — the shape `Pipeline::run_hooked` builds and hands to
/// `before`. Written as a helper rather than a bare bool at each call site
/// because `apply_stop` deliberately takes the whole struct (see its doc).
pub(super) fn stage_info(costs_a_call: bool) -> StageInfo {
    StageInfo {
        pipeline: "resume_max",
        stage: if costs_a_call { "repair" } else { "validate" },
        index: 0,
        total: 1,
        costs_a_call,
    }
}

pub(super) fn report_for(generated: &str, source: &str) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    })
}

pub(super) const CLEAN_SOURCE: &str = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

pub(super) const FABRICATING_DRAFT: &str = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% across 12 teams.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

/// One synthetic reviewable finding, so the anchor cases below can state the
/// exact span they are about instead of hoping the validator emits it.
pub(super) fn fabrication_report(evidence: &str) -> ContentReport {
    ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: crate::validate::Severity::Critical,
            code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
            section: None,
            message: "the draft states a number the source résumé does not".to_string(),
            evidence: Some(evidence.to_string()),
        }],
        metrics: ContentMetrics::default(),
    }
}

pub(super) fn only_fabrication(wrapper: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(wrapper).expect("valid JSON");
    let entries = parsed["resume"]["fabrications"]
        .as_array()
        .expect("one reviewable finding")
        .clone();
    assert_eq!(entries.len(), 1, "fixture declares exactly one finding");
    entries.into_iter().next().expect("the entry")
}

/// Every `issueKey` in a wrapper's résumé review list, in report order.
pub(super) fn fabrication_keys(wrapper: &str) -> Vec<String> {
    let parsed: serde_json::Value = serde_json::from_str(wrapper).expect("valid JSON");
    parsed["resume"]["fabrications"]
        .as_array()
        .expect("the fixture must flag something")
        .iter()
        .map(|entry| entry["issueKey"].as_str().expect("issueKey").to_string())
        .collect()
}

/// The verbatim source of `persist_document`'s body — from its own `fn` line
/// through its own top-level closing brace (column 0, so no NESTED `}`
/// inside the function body can end the scan early; same "opener at a
/// strictly smaller indent" assumption
/// `commands::autopilot::tests::every_record_mutation_goes_through_mutate_record`
/// already documents for this exact idiom). Shared by the two regression
/// tests below so the extraction logic is not duplicated, and `include_str!`
/// makes rustc track the file, so this can never silently read a stale copy.
///
/// **Why a source pin at all.** `persist_document` takes a live `&AppHandle`
/// and this crate has no `tauri::test` mock-app harness (see the doc on
/// `every_record_mutation_goes_through_mutate_record` for the same
/// limitation) — driving it for real is not available. A source pin is the
/// cheapest HONEST guard: it fails the instant the exact line it names is
/// edited away, which is what a mutation check below actually exercises.
pub(super) fn persist_document_source() -> String {
    const SRC: &str = include_str!("../persist.rs");
    let lines: Vec<&str> = SRC.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start() == "pub(super) fn persist_document(")
        .expect("persist_document must still exist under this exact signature line");
    let end = start
        + lines[start..]
            .iter()
            .position(|l| *l == "}")
            .expect("persist_document's own top-level closing brace (column 0)");
    lines[start..=end].join("\n")
}
