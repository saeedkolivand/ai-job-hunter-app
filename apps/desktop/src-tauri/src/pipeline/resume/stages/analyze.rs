//! `analyze_job` — one call, deterministic intent: what does this posting ask
//! for?
//!
//! The candidate is deliberately absent from this turn. An analysis produced
//! while looking at a résumé drifts toward describing that candidate's
//! strengths as the job's requirements, which then makes every downstream
//! "match" self-fulfilling.

use async_trait::async_trait;
use serde_json::json;

use std::collections::HashSet;

use crate::documents::keywords::{
    detected_language, keywords_normalized_list, markdown_to_plain, SHORT_TECH_TERMS, SYNONYMS,
};
use crate::error::{AppError, AppResult};
use crate::pipeline::resume::floor::{store_sound, with_floor};
use crate::pipeline::resume::prompts::{analyze_job_user, ANALYZE_JOB_SYSTEM};
use crate::pipeline::resume::types::JobAnalysis;
use crate::pipeline::resume::{cache, QualityCtx};
use crate::pipeline::Stage;

pub struct AnalyzeJob;

const NAME: &str = "analyze_job";

/// Most keyword-kernel terms promoted to must-haves when both model attempts
/// missed the floor.
const FALLBACK_MUST_HAVES: usize = 8;

/// Fill an analysis' empty must-haves from the keyword kernel; whether it did.
fn apply_keyword_fallback(analysis: &mut JobAnalysis, job_ad: &str, company: &str) -> bool {
    if !analysis.must_have.is_empty() {
        return false;
    }
    analysis.must_have = keyword_must_haves(job_ad, &analysis.role_title, company);
    !analysis.must_have.is_empty()
}

/// Skill-shaped term: a kernel-known tech term (short allowlist or a canonical
/// synonym such as "python"), or one with `+ # .`.
fn techy(token: &str) -> bool {
    SHORT_TECH_TERMS.contains(&token)
        || SYNONYMS.iter().any(|(_, canon)| *canon == token)
        || token.contains(['+', '#', '.'])
}

/// Tokens written capitalised mid-sentence ("... using Kotlin code"): product
/// and technology names, as opposed to sentence-initial words.
fn proper_nouns(plain: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in plain.lines() {
        let mut prev = "";
        for word in line.split_whitespace() {
            let sentence_start = prev.is_empty() || prev.ends_with(['.', '!', '?', ':', '-']);
            if !sentence_start && word.starts_with(char::is_uppercase) {
                out.extend(keywords_normalized_list(word));
            }
            prev = word;
        }
    }
    out
}

/// Deterministic stand-in for the must-haves a degraded analysis failed to
/// extract (#1392): `documents::keywords` terms of the plain-text posting minus
/// the company and role-title words; skill-shaped terms first, then
/// mid-sentence-capitalised names, then most repeated (ties keep document order).
fn keyword_must_haves(job_ad: &str, role_title: &str, company: &str) -> Vec<String> {
    let skip: HashSet<String> = keywords_normalized_list(&format!("{company} {role_title}"))
        .into_iter()
        .collect();
    let plain = markdown_to_plain(job_ad);
    // German capitalises every noun, so capitalisation says nothing there.
    let german = detected_language(&plain) == Some("de");
    let proper = if german {
        HashSet::new()
    } else {
        proper_nouns(&plain)
    };
    let mut counts: Vec<(String, usize)> = Vec::new();
    for token in keywords_normalized_list(&plain) {
        if skip.contains(&token) {
            continue;
        }
        match counts.iter_mut().find(|(t, _)| *t == token) {
            Some((_, n)) => *n += 1,
            None => counts.push((token, 1)),
        }
    }
    // Nothing else tells a German skill from a German noun: keep only
    // skill-shaped terms when the ad has any.
    if german && counts.iter().any(|(t, _)| techy(t)) {
        counts.retain(|(t, _)| techy(t));
    }
    counts.sort_by_key(|(t, n)| (!techy(t), !proper.contains(t), std::cmp::Reverse(*n)));
    counts
        .into_iter()
        .take(FALLBACK_MUST_HAVES)
        .map(|(t, _)| t)
        .collect()
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for AnalyzeJob {
    fn name(&self) -> &'static str {
        "analyze_job"
    }

    /// Cancel drops the in-flight JSON call (closing the HTTP stream) instead of
    /// waiting out the model, which has no cancel check of its own (#1391).
    fn abandon_on_cancel(&self) -> bool {
        true
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
        let mut keyword_fallback = false;
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
                let mut analysis = floored.value;
                // `complete_json` guarantees the response PARSED; it cannot
                // know whether the model answered. Every field is
                // `#[serde(default)]`, so `{}` is a successful parse and an
                // empty analysis — which would then silently produce an
                // evidence map with no requirements and a strategy with no
                // emphasis, all reported as a clean run.
                // Intentionally BEFORE the keyword fallback: a model that returns
                // nothing at all is broken, not weak.
                if analysis.is_empty() {
                    return Err(AppError::Provider(
                        "The model returned no requirements for this posting. Try again, or \
                         pick a larger model."
                            .to_string(),
                    ));
                }
                // Both attempts missed the floor: real must-haves from the
                // keyword kernel beat none (the artifact flags it).
                keyword_fallback = degraded
                    && apply_keyword_fallback(
                        &mut analysis,
                        ctx.input.job_ad,
                        ctx.input.company_name,
                    );
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
                "keywordFallback": keyword_fallback,
            }),
        );
        ctx.analysis = analysis;
        // Starts the cover-letter research lookup, if the run armed one.
        ctx.publish_role();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
