//! `match_evidence` — pure Rust evidence selection, no provider call.
//!
//! The stage used to ask a model to rank the résumé's lines and copy the best
//! one out for each requirement, then Rust overwrote or discarded most of what
//! it returned (the `verbatim` check, the kernel status). Nothing a model
//! produced survived that could not be derived deterministically, so the stage
//! now decides the whole `EvidenceMap` itself, from the candidate's OWN résumé:
//!
//! 1. **The quote is résumé text by construction** — it is one bullet text copied
//!    out of `documents::evidence::extract_evidence`, never model prose, so
//!    there is nothing to blank afterwards.
//! 2. **The status is kernel-decided** against the whole source résumé, as
//!    before, so the pipeline's coverage claim can never disagree with the
//!    match percentage the user already sees on the Jobs page.
//!
//! The REQUIREMENT SET is Rust-owned too: the analysis's must-have ∪
//! nice-to-have, bounded — the same fixed list as ever.
//!
//! Deterministic and free: `costs_a_provider_call` is `false` (no model, no
//! cache — it is cheap to recompute), and every choice below is pinned by the
//! test module at the bottom of this file.

use std::collections::HashSet;

use async_trait::async_trait;
use rust_stemmers::Stemmer;
use serde_json::json;

use crate::documents::evidence::{contains_word, extract_evidence};
use crate::documents::keywords::{keywords, keywords_normalized, languages_align, make_stemmer};
use crate::error::AppResult;
use crate::pipeline::resume::types::{EvidenceItem, EvidenceMap, EvidenceStatus};
use crate::pipeline::resume::QualityCtx;
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
    /// drops tokens of 3 bytes or fewer unless the term is on the
    /// `SHORT_TECH_TERMS` keep-list, so a short term that is NOT listed
    /// (e.g. "Gui") tokenizes to NOTHING. Answering `Missing` for those would
    /// mark the commonest tech requirements as unsupported on a résumé that
    /// names them in every bullet, and `strategy` would then be told not to
    /// emphasize them. The fallback is a word-BOUNDED match (the same
    /// `contains_word` every lexicon comparison in `validate::content` uses),
    /// not a substring — "go" must not match "golang-agnostic prose" or
    /// "Django".
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

/// One source résumé line that can back a quote.
///
/// Tokens and the lowercased text are computed ONCE per stage run and reused
/// across requirements — the cost shape the selection is pinned to: one
/// `extract_evidence` call, then `lines × requirements` set intersections,
/// bounded by `MAX_REQUIREMENTS` on the requirement side.
struct Candidate {
    text: String,
    company: String,
    tokens: HashSet<String>,
    lower: String,
}

impl Candidate {
    fn new(text: String, company: String, kernel: &Kernel) -> Self {
        Self {
            lower: text.to_lowercase(),
            tokens: kernel.tokens(&text),
            text,
            company,
        }
    }
}

/// Requirement-token hits in a line: how many of the requirement's distinct
/// kernel tokens the line's kernel tokens contain. Intersecting TOKEN sets,
/// never characters, is what keeps "go" out of "Django".
///
/// If the requirement tokenizes to nothing (short terms like "Gui" — see
/// [`Kernel::status_for`]), a line scores 1 on a word-BOUNDED hit of the
/// requirement text and 0 otherwise, so a fallback requirement can never match
/// inside a longer word: "gui" must not match "distinguish".
fn score(line: &Candidate, requirement_tokens: &HashSet<String>, requirement_lower: &str) -> usize {
    if requirement_tokens.is_empty() {
        usize::from(!requirement_lower.is_empty() && contains_word(&line.lower, requirement_lower))
    } else {
        line.tokens
            .iter()
            .filter(|token| requirement_tokens.contains(*token))
            .count()
    }
}

/// The advisory 0–3: 0 when there is no quote; 3 when the quote covers ALL of
/// the requirement's tokens (or the short-term fallback matched) AND the line
/// contains an ASCII digit — a quantified result; 2 when it covers all tokens
/// without a digit; 1 when it covers only some.
fn item_strength(
    best: Option<&Candidate>,
    best_score: usize,
    requirement_tokens: &HashSet<String>,
) -> u8 {
    let Some(line) = best else {
        return 0;
    };
    let all = if requirement_tokens.is_empty() {
        best_score > 0 // the short-term fallback matched
    } else {
        best_score == requirement_tokens.len()
    };
    let has_digit = line.text.chars().any(|c| c.is_ascii_digit());
    if all && has_digit {
        3
    } else if all {
        2
    } else {
        1
    }
}

/// Select the evidence map without any model: for each requirement, the single
/// best-supporting résumé line.
///
/// Candidate lines are every bullet of every role (`source_company` = that
/// role's company), then every project bullet (`source_company` = empty), in
/// document order. Best = highest [`score`] > 0; a tie goes to the EARLIER
/// line; no line scoring > 0 yields an empty quote and company. Every non-empty
/// quote is therefore a parsed résumé line by construction, and `strength` is
/// derived from coverage plus a digit — see [`item_strength`].
///
/// Pure (no ctx, no provider, no cache), so every rule is testable without a
/// model — which is the whole point of them being rules.
pub(crate) fn build_evidence(
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
    requirements: &[String],
) -> EvidenceMap {
    let kernel = Kernel::new(source_resume, job_ad, target_language);
    // ONE extraction per stage run, not per requirement.
    let extracted = extract_evidence(source_resume, job_ad);
    let mut candidates: Vec<Candidate> = Vec::new();
    for role in &extracted.roles {
        for bullet in &role.bullets {
            candidates.push(Candidate::new(
                bullet.text.clone(),
                role.company.clone(),
                &kernel,
            ));
        }
    }
    for bullet in &extracted.projects {
        candidates.push(Candidate::new(bullet.text.clone(), String::new(), &kernel));
    }

    let items = requirements
        .iter()
        .map(|requirement| {
            let requirement_tokens = kernel.tokens(requirement);
            let requirement_lower = requirement.trim().to_lowercase();
            // Earlier line wins a tie: only a STRICTLY greater score replaces
            // the best, so the first line with a given score stays unbeaten by
            // later lines with the same score. (`max_by_key` would pick the
            // LAST max — deliberately not used.)
            let mut best: Option<&Candidate> = None;
            let mut best_score = 0usize;
            for candidate in &candidates {
                let candidate_score = score(candidate, &requirement_tokens, &requirement_lower);
                if candidate_score > best_score {
                    best_score = candidate_score;
                    best = Some(candidate);
                }
            }
            EvidenceItem {
                requirement: requirement.clone(),
                status: kernel.status_for(requirement),
                source_quote: best.map(|line| line.text.clone()).unwrap_or_default(),
                source_company: best.map(|line| line.company.clone()).unwrap_or_default(),
                strength: item_strength(best, best_score, &requirement_tokens),
            }
        })
        .collect();

    EvidenceMap { items }
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for MatchEvidence {
    fn name(&self) -> &'static str {
        "match_evidence"
    }

    /// Deterministic — pure Rust, no provider, nothing to cache or bill (the
    /// module doc's "no provider call", as something the run can act on).
    fn costs_a_provider_call(&self) -> bool {
        false
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let requirements = requirement_set(&ctx.analysis.must_have, &ctx.analysis.nice_to_have);

        // Cheap to recompute, so no cache: the map is always in step with the
        // source résumé and the analysis.
        let evidence = build_evidence(
            ctx.input.source_resume,
            ctx.input.job_ad,
            ctx.input.target_language,
            &requirements,
        );

        // Downstream stage cache keys still chain the evidence map, so a
        // changed map invalidates `strategy` and the rest — the one cache-role
        // this stage keeps.
        let json = serde_json::to_string(&evidence).unwrap_or_default();
        ctx.cache_key.extend(&json);
        // The résumé joins the chain HERE (see `StageCacheKey::extend_source`):
        // the map alone can be identical for two different résumés.
        ctx.cache_key.extend_source(ctx.input.source_resume);

        let covered = evidence
            .items
            .iter()
            .filter(|item| item.status == EvidenceStatus::Covered)
            .count();
        let quoted = evidence
            .items
            .iter()
            .filter(|item| !item.source_quote.trim().is_empty())
            .count();
        // Counts only — never a quote, which is résumé text (ADR-027).
        ctx.ledger.record(
            NAME,
            json!({
                "requirements": evidence.items.len(),
                "covered": covered,
                "quoted": quoted,
            }),
        );
        ctx.evidence = evidence;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
