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
use crate::pipeline::resume::types::{CompanyPlan, EvidenceMap, EvidenceStatus, ResumeStrategy};
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
        // The condensed entry names the employers it stands for in its `title`
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
            dates: condensed_span(rest.iter().map(|role| role.dates.as_str())),
            condensed: true,
            ..CompanyPlan::default()
        });
    }
    roster
}

/// The date range ONE condensed group stands for.
///
/// The group holds several roles, so carrying only ONE of their ranges — which
/// is what `rest.last().dates` gave — understates the history by everything
/// between that role and the cap, and the draft prompt renders this verbatim
/// ("with its company, title and dates exactly as given"). A group standing for
/// three jobs that reads as one job's dates is the same gap in a date range
/// that condensing exists to avoid.
///
/// **Spanned by YEAR, not by position.** Every other approach needs to know
/// which end of the list is oldest, and role order is the DOCUMENT's (usually
/// reverse-chronological, but nothing enforces it) — a positional rule silently
/// inverts on an ascending résumé. Four-digit years are the one token every
/// date format this app sees carries (`2019 - 2021`, `January 2021 – March
/// 2023`, `01/2021 – 03/2023`), and min/max over them is order-free.
///
/// Falls back to the last role's own dates when the group carries no year at
/// all, which is the previous behavior and still better than an empty slot.
/// Known limitation, and safe by construction: a non-year end token
/// (`Present`) is not preserved — the condensed group holds the roles PAST the
/// cap, i.e. the oldest, so a current role can only land here on a résumé that
/// lists eight jobs more recent than an ongoing one.
fn condensed_span<'a>(dates: impl Iterator<Item = &'a str>) -> String {
    let mut years: Vec<u32> = Vec::new();
    let mut last = "";
    for entry in dates {
        last = entry.trim();
        let bytes = last.as_bytes();
        for (index, window) in bytes.windows(4).enumerate() {
            // A standalone 4-digit run: not part of a longer number, so
            // "01/2021" yields 2021 and "12345" yields nothing.
            let bounded = window.iter().all(u8::is_ascii_digit)
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_digit)
                && !bytes.get(index + 4).is_some_and(u8::is_ascii_digit);
            if bounded {
                if let Ok(year) = last[index..index + 4].parse::<u32>() {
                    years.push(year);
                }
            }
        }
    }
    match (years.iter().min(), years.iter().max()) {
        (Some(first), Some(latest)) if first != latest => format!("{first} \u{2013} {latest}"),
        (Some(only), _) => only.to_string(),
        _ => last.to_string(),
    }
}

/// Rebuild the model's plan on the fixed roster.
///
/// For each roster entry, take the model's plan for that employer if it named
/// one — **matched on the company NAME, and only on the name** — and keep only
/// the two fields it is allowed to author: `angle` and `emphasis`. Identity
/// comes back from the roster unconditionally.
///
/// The positional fallback this used to carry (`per_company.get(index)`) looked
/// like tolerance for a model that reworded an employer, but it attaches ANY
/// unmatched plan to whatever roster slot shares its index: a model that
/// reorders, drops, or merges entries — which is exactly the model this
/// re-seeding exists to defend against — gets role B's angle written onto role
/// A, and the result is indistinguishable from a correct plan. An unmatched
/// roster entry now gets NO plan, which reads as "the model said nothing about
/// this role" and is the truth.
///
/// **`emphasis` is filtered against the grounded evidence map.** The prompt
/// says a `missing` requirement must not be emphasized anywhere, but a prompt
/// is not a guarantee: an emphasis on a requirement the résumé cannot evidence
/// is an instruction to the DRAFT stage to write something the source does not
/// support — the fabrication the whole pipeline is built to prevent, arriving
/// one stage before validation can call it a Critical. Only requirements the
/// map lists as `covered` or `partial` survive; a term the map never mentions
/// is unverifiable and goes too.
///
/// Pure, so "never drops a role" and "never emphasizes a gap" are tests rather
/// than claims.
///
/// Returns the rebuilt roster AND how many emphasis terms the grounding filter
/// removed. The count is the only trace a drop leaves: the filter matches on the
/// requirement text, so a model that emphasizes `"Kubernetes "` or `"k8s"`
/// against a map listing `"Kubernetes"` loses it silently — indistinguishable,
/// in the artifact, from a model that emphasized nothing. Counts only (ADR-027):
/// the artifact never carries which term went.
pub(crate) fn reseed(
    roster: &[CompanyPlan],
    model: &ResumeStrategy,
    evidence: &EvidenceMap,
) -> (Vec<CompanyPlan>, u32) {
    let mut dropped = 0u32;
    let plans = roster
        .iter()
        .map(|seed| {
            let proposed = model.per_company.iter().find(|plan| {
                !seed.company.trim().is_empty()
                    && plan
                        .company
                        .trim()
                        .eq_ignore_ascii_case(seed.company.trim())
            });
            let emphasis = proposed
                .map(|p| {
                    let kept = grounded_emphasis(&p.emphasis, evidence);
                    dropped += (p.emphasis.len() - kept.len()) as u32;
                    kept
                })
                .unwrap_or_default();
            CompanyPlan {
                company: seed.company.clone(),
                title: seed.title.clone(),
                dates: seed.dates.clone(),
                condensed: seed.condensed,
                angle: proposed.map(|p| p.angle.clone()).unwrap_or_default(),
                emphasis,
            }
        })
        .collect();
    (plans, dropped)
}

/// The subset of `emphasis` the résumé can actually evidence.
fn grounded_emphasis(emphasis: &[String], evidence: &EvidenceMap) -> Vec<String> {
    emphasis
        .iter()
        .filter(|term| {
            let term = term.trim();
            !term.is_empty()
                && evidence.items.iter().any(|item| {
                    item.status != EvidenceStatus::Missing
                        && item.requirement.trim().eq_ignore_ascii_case(term)
                })
        })
        .cloned()
        .collect()
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Strategy {
    fn name(&self) -> &'static str {
        "strategy"
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let roster = seed_company_roster(ctx.input.source_resume, ctx.input.job_ad);

        let key = ctx.stage_cache_key(NAME);
        let cached: Option<ResumeStrategy> = cache::get(ctx.cache, NAME, &key);
        let from_cache = cached.is_some();
        let mut strategy = match cached {
            Some(strategy) => strategy,
            None => {
                let user = format!(
                    "{}\n\n{}",
                    strategy_user(ctx.input.source_resume, &ctx.analysis, &ctx.evidence),
                    company_roster_block(&roster)
                );
                ctx.completer_for(NAME)
                    .complete_json(
                        // The re-ask is a second full provider call; a run
                        // already out of time must not pay for it.
                        ctx.deadline_guard(),
                        &strategy_system(),
                        &user,
                        ResumeStrategy::EXAMPLE,
                        Some(&ResumeStrategy::schema()),
                        ctx.input.effort,
                    )
                    .await?
            }
        };
        // Applied on the cache-hit path too: a cached artifact was grounded
        // against the same source (the key chains it in), but re-seeding is
        // free and makes the invariant hold for every path out of this stage
        // rather than for one of them.
        let (per_company, emphasis_dropped) = reseed(&roster, &strategy, &ctx.evidence);
        strategy.per_company = per_company;

        let json = serde_json::to_string(&strategy).unwrap_or_default();
        if !from_cache {
            cache::put(ctx.cache, NAME, &key, &json);
        }
        ctx.cache_key.extend(&json);
        ctx.ledger.count_call(from_cache);
        // Counts only — never a company name or an angle (ADR-027).
        // `emphasisDropped` is how many emphasis terms the grounding filter
        // removed: a silent drop otherwise looks exactly like a model that
        // emphasized nothing, and a run with a high count is one whose evidence
        // map and strategy disagree about how requirements are spelled.
        ctx.ledger.record(
            "strategy",
            json!({
                "cached": from_cache,
                "companies": strategy.per_company.len(),
                "condensed": strategy.per_company.iter().filter(|p| p.condensed).count(),
                "skillsGroups": strategy.skills_groups.len(),
                "emphasisDropped": emphasis_dropped,
            }),
        );
        ctx.strategy = strategy;
        Ok(())
    }
}
