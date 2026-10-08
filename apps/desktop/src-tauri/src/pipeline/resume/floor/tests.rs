use std::cell::RefCell;

use super::super::cache::{self, StageCacheKey, StageIdentity};
use super::super::types::{CompanyPlan, JobAnalysis, ResumeStrategy};
use super::{store_sound, with_floor};
use crate::error::AppError;
use crate::pipeline::cache::KvCache;

/// ~2.5k chars, shaped like the probe's Android ad.
fn long_ad() -> String {
    "Senior Android Engineer. Required: Kotlin, Jetpack Compose, Coroutines. ".repeat(40)
}

/// The probe's good qwen3.8 answer (counts 13/8/5), trimmed.
fn good_analysis() -> JobAnalysis {
    JobAnalysis {
        role_title: "Senior Android Engineer".into(),
        must_have: vec!["Kotlin".into(), "Jetpack Compose".into()],
        nice_to_have: vec!["Kotlin Multiplatform".into()],
        responsibilities: vec!["Own app performance".into()],
        ..JobAnalysis::default()
    }
}

/// The degraded shape from #1382: no requirements, reasoning in `seniority`.
fn degraded_analysis() -> JobAnalysis {
    JobAnalysis {
        role_title: "Senior Android Engineer".into(),
        seniority: "Let me think about this posting step by step ".repeat(150),
        ..JobAnalysis::default()
    }
}

#[test]
fn analysis_floor_separates_good_from_degraded_on_a_real_ad() {
    let ad = long_ad();
    assert!(!good_analysis().below_floor(&ad));
    assert!(degraded_analysis().below_floor(&ad));
    // #1392: responsibilities alone no longer clear the floor; must-haves alone do.
    let only_resp = JobAnalysis {
        responsibilities: vec!["Ship weekly".into()],
        ..JobAnalysis::default()
    };
    assert!(only_resp.below_floor(&ad));
    let only_must = JobAnalysis {
        must_have: vec!["Kotlin".into()],
        ..JobAnalysis::default()
    };
    assert!(!only_must.below_floor(&ad));
}

#[test]
fn analysis_floor_does_not_apply_to_a_trivial_ad() {
    assert!(!degraded_analysis().below_floor("Android dev wanted."));
}

#[test]
fn strategy_floor_trips_only_on_a_strategy_that_said_nothing() {
    let empty = ResumeStrategy {
        per_company: vec![CompanyPlan::default()],
        ..ResumeStrategy::default()
    };
    assert!(empty.below_floor());
    let angled = ResumeStrategy {
        headline_angle: "Mobile engineer who ships reliability".into(),
        ..ResumeStrategy::default()
    };
    assert!(!angled.below_floor());
    let per_company_only = ResumeStrategy {
        per_company: vec![CompanyPlan {
            emphasis: vec!["Kotlin".into()],
            ..CompanyPlan::default()
        }],
        ..ResumeStrategy::default()
    };
    assert!(!per_company_only.below_floor());
}

/// Records the effort each call was made at and replays a script of answers.
async fn run(
    effort: Option<&'static str>,
    script: Vec<Result<JobAnalysis, ()>>,
) -> (
    crate::error::AppResult<super::Floored<JobAnalysis>>,
    Vec<Option<String>>,
) {
    let ad = long_ad();
    let calls = RefCell::new(Vec::new());
    let script = RefCell::new(script.into_iter());
    let out = with_floor(
        effort,
        |e| {
            calls.borrow_mut().push(e.map(str::to_string));
            let next = script.borrow_mut().next().expect("unexpected extra call");
            async move { next.map_err(|()| AppError::Message("boom".into())) }
        },
        |a| a.below_floor(&ad),
        JobAnalysis::richness,
    )
    .await;
    (out, calls.into_inner())
}

#[tokio::test]
async fn a_floor_miss_retries_once_at_the_default_and_keeps_the_better_result() {
    let (out, calls) = run(
        Some("off"),
        vec![Ok(degraded_analysis()), Ok(good_analysis())],
    )
    .await;
    let out = out.unwrap();
    assert_eq!(calls, vec![Some("off".to_string()), None]);
    assert!(out.retried && !out.degraded);
    assert_eq!(out.value, good_analysis());
}

#[tokio::test]
async fn a_passing_first_answer_makes_one_call() {
    let (out, calls) = run(Some("off"), vec![Ok(good_analysis())]).await;
    let out = out.unwrap();
    assert_eq!(calls.len(), 1);
    assert!(!out.retried && !out.degraded);
}

#[tokio::test]
async fn no_retry_when_no_effort_was_sent() {
    let (out, calls) = run(None, vec![Ok(degraded_analysis())]).await;
    let out = out.unwrap();
    assert_eq!(calls.len(), 1);
    assert!(out.degraded && !out.retried);
}

#[tokio::test]
async fn a_retry_that_is_no_better_or_fails_keeps_the_first_answer() {
    let (out, _) = run(
        Some("off"),
        vec![Ok(degraded_analysis()), Ok(JobAnalysis::default())],
    )
    .await;
    let out = out.unwrap();
    assert!(out.degraded && out.retried);
    assert_eq!(out.value, degraded_analysis());

    let (out, _) = run(Some("off"), vec![Ok(degraded_analysis()), Err(())]).await;
    let out = out.unwrap();
    assert!(out.degraded && out.retried);
    assert_eq!(out.value, degraded_analysis());
}

#[test]
fn a_floor_miss_is_never_written_to_the_stage_cache() {
    let dir = tempfile::TempDir::new().unwrap();
    let kv = KvCache::open(dir.path()).expect("open cache");
    let key = StageCacheKey::new(
        StageIdentity {
            provider: "ollama",
            model: "qwen3.8",
            context_window: None,
            effort: None,
        },
        "s",
    );
    let json = serde_json::to_string(&degraded_analysis()).unwrap();

    store_sound(Some(&kv), "analyze_job", &key, &json, true);
    assert!(cache::get::<JobAnalysis>(Some(&kv), "analyze_job", &key).is_none());

    let good = serde_json::to_string(&good_analysis()).unwrap();
    store_sound(Some(&kv), "analyze_job", &key, &good, false);
    assert_eq!(
        cache::get::<JobAnalysis>(Some(&kv), "analyze_job", &key),
        Some(good_analysis())
    );
}

/// Judged AFTER `reseed`: angles under an employer the roster does not know are
/// dropped by the rebuild, so the stored (and read-back) strategy is empty and
/// must be treated as degraded — and therefore not cached.
#[test]
fn a_strategy_whose_only_content_is_under_an_unmatched_employer_is_degraded_and_not_cached() {
    use super::super::stages::reseed;
    use super::super::types::EvidenceMap;

    let roster = vec![CompanyPlan {
        company: "Acme Payments".into(),
        ..CompanyPlan::default()
    }];
    let raw = ResumeStrategy {
        per_company: vec![CompanyPlan {
            company: "Some Other Employer".into(),
            angle: "Lead with the migration".into(),
            ..CompanyPlan::default()
        }],
        ..ResumeStrategy::default()
    };
    assert!(!raw.below_floor(), "the raw answer looks fine");

    let (per_company, _) = reseed(&roster, &raw, &EvidenceMap::default());
    let stored = ResumeStrategy { per_company, ..raw };
    let degraded = stored.below_floor();
    assert!(degraded, "after the rebuild it said nothing");

    let dir = tempfile::TempDir::new().unwrap();
    let kv = KvCache::open(dir.path()).expect("open cache");
    let key = StageCacheKey::new(
        StageIdentity {
            provider: "ollama",
            model: "qwen3.8",
            context_window: None,
            effort: None,
        },
        "s",
    );
    let json = serde_json::to_string(&stored).unwrap();
    store_sound(Some(&kv), "strategy", &key, &json, degraded);
    assert!(cache::get::<ResumeStrategy>(Some(&kv), "strategy", &key).is_none());
}
