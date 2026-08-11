//! `match_evidence` — one call, and then two Rust passes the model cannot
//! influence.
//!
//! The model's job is RANKING and QUOTING. Two things are taken away from it
//! afterwards, because both are claims about the candidate rather than about
//! presentation:
//!
//! 1. **The quote must be verbatim** (`super::verbatim`) or it is blanked. Not
//!    repaired — see that module.
//! 2. **The status is overwritten** from the `documents::keywords` kernel: the
//!    same tokenizer, stemmer and `SYNONYMS` behind `score_one` and
//!    `keyword_coverage`. A model calling a missing skill "covered" would make
//!    the pipeline's own coverage claim disagree with the match percentage the
//!    user is looking at on the Jobs page.
//!
//! The REQUIREMENT SET is Rust-owned too: it is the analysis's
//! must-have ∪ nice-to-have, so a model that invents a requirement adds
//! nothing, and one that skips a requirement gets an honest empty item instead
//! of silence.

use std::collections::HashSet;

use async_trait::async_trait;
use rust_stemmers::Stemmer;
use serde_json::json;

use crate::documents::evidence::contains_word;
use crate::documents::keywords::{keywords, keywords_normalized, languages_align, make_stemmer};
use crate::error::AppResult;
use crate::pipeline::resume::prompts::{match_evidence_system, match_evidence_user};
use crate::pipeline::resume::types::{EvidenceItem, EvidenceMap, EvidenceStatus};
use crate::pipeline::resume::{cache, QualityCtx};
use crate::pipeline::Stage;
use crate::validate::content::normalize_language;

pub struct MatchEvidence;

const NAME: &str = "match_evidence";

/// Ceiling on how many requirements one evidence map may carry.
///
/// The list comes from MODEL output (the analysis's two requirement arrays), so
/// it needs a bound for the same reason `Budget::max_sections` does: a model
/// that emits 400 "requirements" would otherwise turn one artifact into a
/// 400-entry blob that rides into every later prompt. Sized well above any real
/// posting — the longest genuine must-have list observed is under 20.
const MAX_REQUIREMENTS: usize = 40;

/// The kernel's verdict on ONE requirement, decided from the SOURCE résumé.
struct Kernel {
    aligned: bool,
    stemmer: Stemmer,
    source_tokens: HashSet<String>,
    source_lower: String,
}

impl Kernel {
    fn new(source_resume: &str, job_ad: &str, target_language: &str) -> Self {
        // The same alignment decision `validate::content::Analysis` takes, from
        // the same kernel: stem BOTH sides or neither. Stemming one side alone
        // mutates language-neutral tech tokens on that side only and matches
        // nothing.
        let aligned = languages_align(job_ad, &normalize_language(target_language));
        let stemmer = make_stemmer(job_ad);
        let source_tokens = if aligned {
            keywords(source_resume, &stemmer)
        } else {
            keywords_normalized(source_resume)
        };
        Self {
            aligned,
            stemmer,
            source_tokens,
            source_lower: source_resume.to_lowercase(),
        }
    }

    fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords(text, &self.stemmer)
        } else {
            keywords_normalized(text)
        }
    }

    /// Covered / partial / missing for one requirement.
    ///
    /// The short-term fallback is load-bearing: the kernel's keyword filter
    /// drops tokens of 3 bytes or fewer, so "Go", "AWS", "CI" and "SQL"
    /// tokenize to NOTHING. Answering `Missing` for those would mark the
    /// commonest tech requirements as unsupported on a résumé that names them
    /// in every bullet, and `strategy` would then be told not to emphasize
    /// them. The fallback is a word-BOUNDED match (the same `contains_word`
    /// every lexicon comparison in `validate::content` uses), not a substring —
    /// "go" must not match "golang-agnostic prose" or "Django".
    fn status_for(&self, requirement: &str) -> EvidenceStatus {
        let tokens = self.tokens(requirement);
        if tokens.is_empty() {
            let needle = requirement.trim().to_lowercase();
            return if !needle.is_empty() && contains_word(&self.source_lower, &needle) {
                EvidenceStatus::Covered
            } else {
                EvidenceStatus::Missing
            };
        }
        let hits = tokens
            .iter()
            .filter(|token| self.source_tokens.contains(*token))
            .count();
        if hits == tokens.len() {
            EvidenceStatus::Covered
        } else if hits > 0 {
            EvidenceStatus::Partial
        } else {
            EvidenceStatus::Missing
        }
    }
}

/// The Rust-owned requirement list: must-have first, then nice-to-have,
/// de-duplicated case-insensitively and bounded.
fn requirement_set(must_have: &[String], nice_to_have: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    must_have
        .iter()
        .chain(nice_to_have.iter())
        .map(|r| r.trim())
        .filter(|r| !r.is_empty())
        .filter(|r| seen.insert(r.to_lowercase()))
        .take(MAX_REQUIREMENTS)
        .map(str::to_string)
        .collect()
}

/// Rebuild the evidence map on the Rust-owned requirement list, keeping only
/// what the model is allowed to contribute.
///
/// Pure (no provider, no cache) so the drop-and-overwrite rules are testable
/// without a model — which is the whole point of them being rules.
pub(crate) fn ground(
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
    requirements: &[String],
    model: EvidenceMap,
) -> (EvidenceMap, usize) {
    let kernel = Kernel::new(source_resume, job_ad, target_language);
    let mut dropped = 0usize;
    let items = requirements
        .iter()
        .map(|requirement| {
            let mut item = model
                .items
                .iter()
                .find(|item| item.requirement.trim().eq_ignore_ascii_case(requirement))
                .cloned()
                .unwrap_or_default();
            item.requirement = requirement.clone();
            if !item.source_quote.trim().is_empty()
                && !super::verbatim::keep_or_blank(source_resume, &mut item.source_quote)
            {
                dropped += 1;
                // The attribution rides with the quote: an employer name kept
                // beside a discarded quote is a claim with nothing behind it.
                item.source_company.clear();
            }
            item.status = kernel.status_for(requirement);
            item.strength = item.strength.min(3);
            item
        })
        .collect::<Vec<EvidenceItem>>();
    (EvidenceMap { items }, dropped)
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for MatchEvidence {
    fn name(&self) -> &'static str {
        "match_evidence"
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let requirements = requirement_set(&ctx.analysis.must_have, &ctx.analysis.nice_to_have);

        // The CACHED value is the already-grounded artifact, so a hit skips the
        // provider call AND the two Rust passes — which is safe precisely
        // because the key chains in the source résumé and the analysis, the
        // only two inputs those passes read.
        let cached: Option<EvidenceMap> = cache::get(ctx.cache, NAME, &ctx.cache_key);
        let from_cache = cached.is_some();
        let (evidence, dropped) = match cached {
            Some(evidence) => (evidence, 0),
            None => {
                let raw: EvidenceMap = ctx
                    .completer
                    .complete_json(
                        &match_evidence_system(),
                        &match_evidence_user(ctx.input.source_resume, &ctx.analysis),
                        EvidenceMap::EXAMPLE,
                        Some(&EvidenceMap::schema()),
                    )
                    .await?;
                ground(
                    ctx.input.source_resume,
                    ctx.input.job_ad,
                    ctx.input.target_language,
                    &requirements,
                    raw,
                )
            }
        };

        let json = serde_json::to_string(&evidence).unwrap_or_default();
        if !from_cache {
            cache::put(ctx.cache, NAME, &ctx.cache_key, &json);
        }
        ctx.cache_key.extend(&json);
        ctx.ledger.count_call(from_cache);
        let covered = evidence
            .items
            .iter()
            .filter(|item| item.status == EvidenceStatus::Covered)
            .count();
        // Counts only — never a quote, which is résumé text (ADR-027).
        ctx.ledger.record(
            "match_evidence",
            json!({
                "cached": from_cache,
                "requirements": evidence.items.len(),
                "covered": covered,
                "quotesDropped": dropped,
            }),
        );
        ctx.evidence = evidence;
        Ok(())
    }
}
