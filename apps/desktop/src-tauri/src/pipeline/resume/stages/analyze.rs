//! `analyze_job` — one call, deterministic intent: what does this posting ask
//! for?
//!
//! The candidate is deliberately absent from this turn. An analysis produced
//! while looking at a résumé drifts toward describing that candidate's
//! strengths as the job's requirements, which then makes every downstream
//! "match" self-fulfilling.

use async_trait::async_trait;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::pipeline::resume::floor::{store_sound, with_floor};
use crate::pipeline::resume::prompts::{analyze_job_user, ANALYZE_JOB_SYSTEM};
use crate::pipeline::resume::types::JobAnalysis;
use crate::pipeline::resume::{cache, QualityCtx};
use crate::pipeline::Stage;

pub struct AnalyzeJob;

const NAME: &str = "analyze_job";

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for AnalyzeJob {
    fn name(&self) -> &'static str {
        "analyze_job"
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        // Bound to the model THIS stage runs on, not to the run's default —
        // see `QualityCtx::stage_cache_key`.
        let key = ctx.stage_cache_key(NAME);
        // A below-floor row is a poisoned write from before the floor existed
        // (#1382): treat it as a miss instead of serving it for a week.
        let cached: Option<JobAnalysis> = cache::get::<JobAnalysis>(ctx.cache, NAME, &key)
            .filter(|a| !a.below_floor(ctx.input.job_ad));
        let from_cache = cached.is_some();
        let mut degraded = false;
        let mut retried = false;
        let analysis = match cached {
            Some(analysis) => analysis,
            None => {
                let floored = with_floor(
                    // Mechanical stage: the user's effort, else the lowest tier.
                    ctx.stage_effort(NAME),
                    |effort| {
                        let ctx = &*ctx;
                        async move {
                            ctx.completer_for(NAME)
                                .complete_json::<JobAnalysis>(
                                    // The re-ask is a second full provider call; a
                                    // run already out of time must not pay for it.
                                    ctx.deadline_guard(),
                                    ANALYZE_JOB_SYSTEM,
                                    &analyze_job_user(ctx.input.job_ad),
                                    JobAnalysis::EXAMPLE,
                                    Some(&JobAnalysis::schema()),
                                    effort,
                                )
                                .await
                        }
                    },
                    |a| a.below_floor(ctx.input.job_ad),
                    JobAnalysis::richness,
                )
                .await?;
                (degraded, retried) = (floored.degraded, floored.retried);
                let analysis = floored.value;
                // `complete_json` guarantees the response PARSED; it cannot
                // know whether the model answered. Every field is
                // `#[serde(default)]`, so `{}` is a successful parse and an
                // empty analysis — which would then silently produce an
                // evidence map with no requirements and a strategy with no
                // emphasis, all reported as a clean run.
                if analysis.is_empty() {
                    return Err(AppError::Provider(
                        "The model returned no requirements for this posting. Try again, or \
                         pick a larger model."
                            .to_string(),
                    ));
                }
                analysis
            }
        };

        let json = serde_json::to_string(&analysis).unwrap_or_default();
        // Never cache a floor miss: one bad call must not poison a week of runs.
        store_sound(ctx.cache, NAME, &key, &json, from_cache || degraded);
        ctx.cache_key.extend(&json);
        ctx.ledger.count_call(from_cache);
        if retried {
            ctx.ledger.count_call(false);
        }
        // Counts only — never a requirement's text (ADR-027).
        ctx.ledger.record(
            "analyze_job",
            json!({
                "cached": from_cache,
                "mustHave": analysis.must_have.len(),
                "niceToHave": analysis.nice_to_have.len(),
                "retried": retried,
                "belowFloor": degraded,
            }),
        );
        ctx.analysis = analysis;
        // Starts the cover-letter research lookup, if the run armed one.
        ctx.publish_role();
        Ok(())
    }
}
