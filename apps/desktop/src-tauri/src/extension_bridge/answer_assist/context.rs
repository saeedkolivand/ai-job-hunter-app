//! Context resolution (URL-matched Application).

use crate::applications::{normalize_job_url, Application, ApplicationStore};
use crate::salary_research::SalaryRange;

/// Resolve the URL-matched Application, the SAME canonicalize + normalize
/// path `resolve_answers_save`/`resolve_match_live` use, so an `answer.assist`
/// on the same page a "Check fit"/import ran against hits the identical row.
/// `None` url, or no match, both fall back to generic grounding — never an
/// error (a missing match is normal, not a refusal condition for this verb).
pub(super) fn resolve_context(store: &ApplicationStore, url: Option<&str>) -> Option<Application> {
    let url = url?;
    let canonical = crate::scraping::scrape_url::canonical_job_url(url);
    let effective = canonical.as_deref().unwrap_or(url);
    let normalized = normalize_job_url(effective);
    if normalized.is_empty() {
        return None;
    }
    store.find_by_job_url(&normalized)
}

/// The matched Application's OWN scraped salary range, when it has one —
/// takes precedence over a market lookup (the employer's own stated figure
/// for THIS posting, not a market estimate). Pure — directly unit-testable
/// against a synthetic `Application`.
pub(super) fn scraped_salary_range(app_ctx: Option<&Application>) -> Option<SalaryRange> {
    let a = app_ctx?;
    let (min, max) = (a.salary_min?, a.salary_max?);
    Some(SalaryRange {
        min: min.max(0.0).round() as u32,
        max: max.max(0.0).round() as u32,
        currency: a.salary_currency.clone().unwrap_or_default(),
    })
}
