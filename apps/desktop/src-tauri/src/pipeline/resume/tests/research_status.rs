//! #1500: the run reports how its company research ended, whichever path ran it.
//!
//! The crate has no mock `AppHandle`, so `research_brief` itself (which needs a `Completer`)
//! cannot run in a test. Everything it decides is in the pure seams driven below; the one thing
//! left to a scan is that `research_brief` calls them, and that scan is scoped to its body.

use super::super::stages::{letter_artifact, require_answered, research_refused, research_settled};
use super::super::types::JobAnalysis;
use super::super::RunLedger;
use crate::ai_provider::SearchBackend;
use crate::commands::resume_pipeline::hooks::note_provider_failure;
use crate::error::{AppError, AppResult, FailureKind};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::enrichment::{EnrichmentResult, ResearchStatus};
use serde_json::json;

fn result(content: &str, status: ResearchStatus) -> EnrichmentResult {
    EnrichmentResult {
        key: "Example Corp".into(),
        content: content.into(),
        status,
        backend: None,
    }
}

/// The run's research status names the backend that served it, so a letter written from a
/// fallback is attributable. Mutation check: drop the `backend` insert in `RunLedger::metrics`
/// and this fails; a `None`/`none` backend must add nothing.
#[test]
fn the_ledger_names_the_backend_that_served_the_research() {
    let ledger = RunLedger::new();
    let mut served = result("a brief", ResearchStatus::Ok);
    served.backend = Some(SearchBackend::Searxng);
    research_settled(&ledger, served);
    assert_eq!(
        ledger.metrics()["research"],
        json!({ "status": "ok", "backend": "searxng" })
    );

    let mut failed = result("", ResearchStatus::Failed(Some(FailureKind::Billing)));
    failed.backend = Some(SearchBackend::Exa);
    research_settled(&ledger, failed);
    assert_eq!(
        ledger.metrics()["research"],
        json!({ "status": "failed", "errorKind": "billing", "backend": "exa" })
    );

    let mut nothing = result("", ResearchStatus::Empty);
    nothing.backend = Some(SearchBackend::None);
    research_settled(&ledger, nothing);
    assert_eq!(ledger.metrics()["research"], json!({ "status": "empty" }));
}

/// Mutation check: drop the `research` entry from `RunLedger::metrics` and `get` can no longer
/// tell a letter written without research because the account ran out of credit.
#[test]
fn the_ledger_carries_research_status_and_kind_into_the_run_metrics() {
    let ledger = RunLedger::new();
    assert!(
        ledger.metrics().get("research").is_none(),
        "none until asked"
    );

    research_settled(
        &ledger,
        result("", ResearchStatus::Failed(Some(FailureKind::Billing))),
    );
    assert_eq!(
        ledger.metrics()["research"],
        json!({ "status": "failed", "errorKind": "billing" })
    );

    research_settled(&ledger, result("", ResearchStatus::Empty));
    assert_eq!(ledger.metrics()["research"], json!({ "status": "empty" }));
}

/// The settled brief reaches the letter unchanged, and a failed lookup never fails the letter.
#[test]
fn a_settled_lookup_hands_its_brief_on_and_a_refused_one_hands_on_nothing() {
    let ledger = RunLedger::new();
    assert_eq!(
        research_settled(&ledger, result("a brief", ResearchStatus::Ok)),
        "a brief"
    );
    assert_eq!(ledger.research(), Some(ResearchStatus::Ok));

    let refused = RunLedger::new();
    assert_eq!(research_refused(&refused), "");
    assert_eq!(
        refused.research(),
        Some(ResearchStatus::Failed(Some(FailureKind::RateLimited))),
        "our own admission refusal is a failed lookup, not silence"
    );
}

/// The letter stage's artifact carries the status beside the existing counters.
#[test]
fn the_letter_artifact_reports_the_research_status() {
    let with = letter_artifact(
        "Dear team",
        true,
        0,
        Some(ResearchStatus::Failed(Some(FailureKind::Auth))),
    );
    assert_eq!(
        with["researchStatus"],
        json!({ "status": "failed", "errorKind": "auth" })
    );
    assert_eq!(with["researchAttempted"], true);
    assert_eq!(with["researchBriefChars"], 0);
    assert!(letter_artifact("Dear team", false, 0, None)
        .get("researchStatus")
        .is_none());
}

/// The body of `fn <name>` in `source`, brace-matched, comments dropped, whitespace removed.
fn body_of(source: &str, name: &str) -> String {
    let start = source.find(name).unwrap_or_else(|| panic!("{name} exists"));
    let open = start + source[start..].find('{').expect("body");
    let mut depth = 0;
    let mut end = open;
    for (i, c) in source[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    source[open..=end]
        .lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .flat_map(str::split_whitespace)
        .collect()
}

/// `research_brief` needs a `Completer`, so what it must DO is scanned in its own body only:
/// refusals and results both go through the seams tested above, and the early lookup passes the
/// run's ledger. Mutation check: delete either call, or the `&ledger` argument, and this fails.
#[test]
fn research_brief_routes_every_outcome_through_the_seams() {
    let brief = body_of(
        include_str!("../stages/cover_letter.rs"),
        "pub(crate) async fn research_brief",
    );
    assert!(brief.contains("returnresearch_refused(ledger);"));
    assert!(brief.contains("research_settled(ledger,result)"));

    let early = body_of(
        include_str!("../early_research.rs"),
        "pub fn start_early_research",
    );
    assert!(early.contains("research_brief(completer,&ledger,"));
}

/// "The model returned no requirements" is the model's output being unusable: not a `Provider`
/// error (which the hooks map to a `provider_error` stop) but `model_output`.
#[test]
fn an_empty_analysis_is_a_model_output_failure_not_a_provider_stop() {
    let err = require_answered(&JobAnalysis::default()).expect_err("empty is refused");
    assert!(matches!(err, AppError::Parse(_)), "{err:?}");
    assert_eq!(FailureKind::of(&err), Some(FailureKind::ModelOutput));

    let ledger = RunLedger::new();
    let outcome: AppResult<()> = Err(err);
    note_provider_failure(&ledger, &outcome, false);
    assert_eq!(ledger.stopped(), None);

    let answered = JobAnalysis {
        role_title: "Engineer".into(),
        ..JobAnalysis::default()
    };
    assert!(require_answered(&answered).is_ok());
}

/// A real provider failure still records its stop reason.
#[test]
fn only_a_provider_failure_records_a_provider_error_stop() {
    let ledger = RunLedger::new();
    let outcome: AppResult<()> = Err(AppError::Billing("out of credit".into()));
    note_provider_failure(&ledger, &outcome, false);
    assert_eq!(ledger.stopped(), Some(StoppedReason::ProviderError));
}
