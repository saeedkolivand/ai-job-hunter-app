//! #1500: the run reports how its company research ended, whichever path ran it.
//!
//! The crate has no mock `AppHandle`, so no test can run `research_brief` or the letter stage;
//! the ledger half is behavioural and the wiring half is pinned by source scan (same approach
//! as `early_research_wiring_guard`).

use super::super::RunLedger;
use crate::error::{AppError, FailureKind};
use crate::pipeline::enrichment::ResearchStatus;
use serde_json::json;

/// Mutation check: drop the `research` entry from `RunLedger::metrics` and `get` can no longer
/// tell a letter written without research because the account ran out of credit.
#[test]
fn the_ledger_carries_research_status_and_kind_into_the_run_metrics() {
    let ledger = RunLedger::new();
    assert!(
        ledger.metrics().get("research").is_none(),
        "none until asked"
    );

    let kind = FailureKind::of(&AppError::Provider("ollama-cloud 402: add credits".into()));
    ledger.note_research(ResearchStatus::Failed(kind));
    assert_eq!(
        ledger.metrics()["research"],
        json!({ "status": "failed", "errorKind": "billing" })
    );

    ledger.note_research(ResearchStatus::Empty);
    assert_eq!(ledger.metrics()["research"], json!({ "status": "empty" }));
    assert_eq!(ledger.research(), Some(ResearchStatus::Empty));
}

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .flat_map(str::split_whitespace)
        .collect()
}

/// Both research paths (the early lookup and the inline fallback) funnel through
/// `research_brief`, so it is the one place that must record. Mutation check: delete either
/// `note_research` call, or stop passing the ledger from the early lookup, and this fails.
#[test]
fn every_research_path_records_its_outcome() {
    let stage = code_only(include_str!("../stages/cover_letter.rs"));
    assert!(
        stage.contains("ledger.note_research(result.status)"),
        "enrich result recorded"
    );
    assert!(
        stage.contains("ledger.note_research(ResearchStatus::Failed("),
        "an admission refusal is a failed lookup, not silence"
    );
    assert!(
        stage.contains("research_brief(completer,&ctx.ledger,"),
        "inline path passes the ledger"
    );

    let early = code_only(include_str!("../early_research.rs"));
    assert!(
        early.contains("research_brief(completer,&ledger,"),
        "early path passes the ledger"
    );

    assert!(
        stage.contains(r#"artifact["researchStatus"]=research.wire()"#),
        "the letter stage's artifact reports the status"
    );
}

/// "The model returned no requirements" is the model's output being unusable, not the provider
/// failing: it must not be a `Provider` error (which the hooks map to `provider_error`).
#[test]
fn an_empty_analysis_is_a_model_output_failure() {
    let analyze = code_only(include_str!("../stages/analyze.rs"));
    let at = analyze
        .find("AppError::Parse(\"Themodelreturnednorequirements")
        .expect("Parse");
    assert!(at > 0);
    assert!(!analyze.contains("AppError::Provider(\"Themodelreturnednorequirements"));
    assert_eq!(
        FailureKind::of(&AppError::Parse(
            "The model returned no requirements".into()
        )),
        Some(FailureKind::ModelOutput)
    );
}

/// The stop reason agrees with the kind: a model that answered nothing usable is not recorded
/// as a `provider_error` stop, while a real provider failure still is.
#[test]
fn only_a_provider_failure_records_a_provider_error_stop() {
    use crate::commands::resume_pipeline::hooks::note_provider_failure;
    use crate::pipeline::budget::StoppedReason;

    let empty_analysis = RunLedger::new();
    let err: crate::error::AppResult<()> =
        Err(AppError::Parse("The model returned no requirements".into()));
    note_provider_failure(&empty_analysis, &err, false);
    assert_eq!(empty_analysis.stopped(), None);

    let out_of_credit = RunLedger::new();
    let err: crate::error::AppResult<()> =
        Err(AppError::Provider("ollama-cloud 402: credits".into()));
    note_provider_failure(&out_of_credit, &err, false);
    assert_eq!(out_of_credit.stopped(), Some(StoppedReason::ProviderError));
}
