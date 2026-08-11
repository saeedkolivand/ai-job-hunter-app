//! `strategy` — one call, and a structural guarantee the model cannot break:
//! **never drop a role.**
//!
//! A dropped employment entry is a `factual.dropped_role` Critical downstream,
//! but catching it in validation is the wrong place to rely on: by then the
//! draft has already been written around a history that is missing a job, and
//! the repair loop would have to reconstruct it. So the roster is SEEDED from
//! the parsed source résumé before the call, and the answer is REBUILT on that
//! roster after it — the output has exactly the roster's entries, in the
//! roster's order, with the roster's company/title/dates, whatever the model
//! returned.

use async_trait::async_trait;
use serde_json::json;

use crate::documents::evidence::extract_evidence;
use crate::error::AppResult;
use crate::pipeline::resume::prompts::{company_roster_block, strategy_system, strategy_user};
use crate::pipeline::resume::types::{CompanyPlan, ResumeStrategy};
use crate::pipeline::resume::{cache, QualityCtx};
use crate::pipeline::Stage;

pub struct Strategy;

const NAME: &str = "strategy";

/// How many employers get their own plan. Past this, every remaining role joins
/// ONE condensed "earlier roles" entry — condensed, never dropped, because the
/// gap a missing job leaves in a date range is the first thing a recruiter
/// notices. Eight covers a full career at typical tenure; the condensed group
/// is what keeps a twenty-year one honest.
pub const MAX_COMPANY_PLANS: usize = 8;

/// The label the condensed group carries in place of an employer name. Not an
/// employer, and deliberately not shaped like one.
const CONDENSED_LABEL: &str = "Earlier roles";

/// Build the fixed roster from the SOURCE résumé.
///
/// `documents::evidence::extract_evidence` is the reader — the same one the
/// agent's evidence tool and the trim panel use — so "what counts as an
/// employment entry" has one answer in this codebase. Roles it could not
/// attribute carry an empty company; they still take a slot, because a bullet
/// with no employer is still work the candidate did.
pub fn seed_company_roster(source_resume: &str, job_ad: &str) -> Vec<CompanyPlan> {
    let roles = extract_evidence(source_resume, job_ad).roles;
    let mut roster: Vec<CompanyPlan> = roles
        .iter()
        .take(MAX_COMPANY_PLANS)
        .map(|role| CompanyPlan {
            company: role.company.clone(),
            title: role.title.clone(),
            dates: role.dates.clone(),
            ..CompanyPlan::default()
        })
        .collect();
    if roles.len() > MAX_COMPANY_PLANS {
        let rest = &roles[MAX_COMPANY_PLANS..];
        // The condensed entry names the employers it stands for in its `dates`
        // slot rather than dropping them: the reader has to be able to see that
        // the history continues, and the draft prompt renders what it is given.
        let companies: Vec<&str> = rest
            .iter()
            .map(|role| role.company.trim())
            .filter(|company| !company.is_empty())
            .collect();
        roster.push(CompanyPlan {
            company: CONDENSED_LABEL.to_string(),
            title: companies.join(", "),
            dates: rest
                .last()
                .map(|role| role.dates.clone())
                .unwrap_or_default(),
            condensed: true,
            ..CompanyPlan::default()
        });
    }
    roster
}

/// Rebuild the model's plan on the fixed roster.
///
/// For each roster entry, take the model's plan for that employer if it named
/// one (matched on the company, falling back to POSITION for the ordinary case
/// where a model reworded an employer it was told not to touch), and keep only
/// the two fields it is allowed to author: `angle` and `emphasis`. Identity
/// comes back from the roster unconditionally.
///
/// Pure, so "never drops a role" is a test rather than a claim.
pub(crate) fn reseed(roster: &[CompanyPlan], model: &ResumeStrategy) -> Vec<CompanyPlan> {
    roster
        .iter()
        .enumerate()
        .map(|(index, seed)| {
            let proposed = model
                .per_company
                .iter()
                .find(|plan| {
                    !seed.company.trim().is_empty()
                        && plan
                            .company
                            .trim()
                            .eq_ignore_ascii_case(seed.company.trim())
                })
                .or_else(|| model.per_company.get(index));
            CompanyPlan {
                company: seed.company.clone(),
                title: seed.title.clone(),
                dates: seed.dates.clone(),
                condensed: seed.condensed,
                angle: proposed.map(|p| p.angle.clone()).unwrap_or_default(),
                emphasis: proposed.map(|p| p.emphasis.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Strategy {
    fn name(&self) -> &'static str {
        "strategy"
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let roster = seed_company_roster(ctx.input.source_resume, ctx.input.job_ad);

        let cached: Option<ResumeStrategy> = cache::get(ctx.cache, NAME, &ctx.cache_key);
        let from_cache = cached.is_some();
        let mut strategy = match cached {
            Some(strategy) => strategy,
            None => {
                let user = format!(
                    "{}\n\n{}",
                    strategy_user(ctx.input.source_resume, &ctx.analysis, &ctx.evidence),
                    company_roster_block(&roster)
                );
                ctx.completer
                    .complete_json(
                        &strategy_system(),
                        &user,
                        ResumeStrategy::EXAMPLE,
                        Some(&ResumeStrategy::schema()),
                    )
                    .await?
            }
        };
        // Applied on the cache-hit path too: a cached artifact was grounded
        // against the same source (the key chains it in), but re-seeding is
        // free and makes the invariant hold for every path out of this stage
        // rather than for one of them.
        strategy.per_company = reseed(&roster, &strategy);

        let json = serde_json::to_string(&strategy).unwrap_or_default();
        if !from_cache {
            cache::put(ctx.cache, NAME, &ctx.cache_key, &json);
        }
        ctx.cache_key.extend(&json);
        ctx.ledger.count_call(from_cache);
        // Counts only — never a company name or an angle (ADR-027).
        ctx.ledger.record(
            "strategy",
            json!({
                "cached": from_cache,
                "companies": strategy.per_company.len(),
                "condensed": strategy.per_company.iter().filter(|p| p.condensed).count(),
                "skillsGroups": strategy.skills_groups.len(),
            }),
        );
        ctx.strategy = strategy;
        Ok(())
    }
}
