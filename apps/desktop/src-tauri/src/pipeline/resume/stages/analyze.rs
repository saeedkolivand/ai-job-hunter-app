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
        let cached: Option<JobAnalysis> = cache::get(ctx.cache, NAME, &ctx.cache_key);
        let from_cache = cached.is_some();
        let analysis = match cached {
            Some(analysis) => analysis,
            None => {
                let analysis: JobAnalysis = ctx
                    .completer
                    .complete_json(
                        ANALYZE_JOB_SYSTEM,
                        &analyze_job_user(ctx.input.job_ad),
                        JobAnalysis::EXAMPLE,
                        Some(&JobAnalysis::schema()),
                    )
                    .await?;
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
        if !from_cache {
            cache::put(ctx.cache, NAME, &ctx.cache_key, &json);
        }
        ctx.cache_key.extend(&json);
        ctx.ledger.count_call(from_cache);
        // Counts only — never a requirement's text (ADR-027).
        ctx.ledger.record(
            "analyze_job",
            json!({
                "cached": from_cache,
                "mustHave": analysis.must_have.len(),
                "niceToHave": analysis.nice_to_have.len(),
                "redFlags": analysis.red_flags.len(),
            }),
        );
        ctx.analysis = analysis;
        Ok(())
    }
}
