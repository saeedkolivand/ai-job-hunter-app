//! SmartRecruiters — public per-company postings API
//!
//! Endpoint: `https://api.smartrecruiters.com/v1/companies/{company}/postings?limit=100`
//! Supports an optional `q` keyword param when `input.query` is non-empty.
//! No global keyword-only search — requires a company slug. The engine skips
//! this board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_json, strip_html};
use super::super::types::{
    BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode, WorkType,
};
use super::common::{ats_all_fetches_failed, normalize_companies};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Location {
    city: Option<String>,
    country: Option<String>,
    remote: Option<bool>,
    hybrid: Option<bool>,
}

/// `locationType` request-param spelling — `ONSITE`, no underscore/hyphen,
/// unlike the wire VALUE this board reports back (see
/// [`smartrecruiters_work_type`]'s doc).
fn smartrecruiters_location_type_param(wt: WorkType) -> &'static str {
    match wt {
        WorkType::Remote => "REMOTE",
        WorkType::Hybrid => "HYBRID",
        WorkType::OnSite => "ONSITE",
    }
}

/// Builds the SmartRecruiters postings-list URL: company slug, optional
/// keyword, and zero-or-more `&locationType=` params — one per requested
/// work type, in order. This is the exact shape the repeatable-param OR
/// semantics were live-verified against (no filter 367 = REMOTE 3 + HYBRID
/// 36 + ONSITE 328; `&locationType=REMOTE&locationType=HYBRID` → 39 = 3+36;
/// all three → 367, the unfiltered total — see
/// `.claude/scratch/work-type-filter.md`, "SmartRecruiters repeatable
/// `locationType`", verified 2026-08-22).
///
/// De-dupes `work_types` itself (first-seen order) rather than trusting the
/// caller to have already called [`crate::scraping::types::BoardSearchInput::work_type_spec`] —
/// `WorkType` has exactly 3 variants, so de-duping bounds the output to at
/// most 3 `&locationType=` occurrences no matter how many (possibly
/// duplicated) entries are passed in. CWE-770 defense-in-depth: a future
/// caller regression that reverts to a raw, un-deduped work-types field must
/// not reopen the URL-amplification vector this function alone can still
/// close.
fn build_list_url(company: &str, keyword: &str, work_types: &[WorkType]) -> String {
    let mut url = if keyword.is_empty() {
        format!(
            "https://api.smartrecruiters.com/v1/companies/{}/postings?limit=100",
            urlencoding::encode(company)
        )
    } else {
        format!(
            "https://api.smartrecruiters.com/v1/companies/{}/postings?limit=100&q={}",
            urlencoding::encode(company),
            urlencoding::encode(keyword)
        )
    };
    let mut seen = std::collections::HashSet::with_capacity(work_types.len());
    for wt in work_types.iter().copied().filter(|wt| seen.insert(*wt)) {
        url.push_str("&locationType=");
        url.push_str(smartrecruiters_location_type_param(wt));
    }
    url
}

/// Map SmartRecruiters' `location.remote` / `location.hybrid` booleans to a
/// declared work type, precedence **hybrid > remote > on-site**.
///
/// **`OnSite` requires POSITIVE evidence on BOTH sides — both keys present
/// AND both `false`.** A `Some(bool)` on one side with the other side absent
/// is NOT enough to conclude on-site: the absence of one key must never
/// manufacture a positive `OnSite` declaration from the other. This board's
/// `locationType` filter partitions its corpus exactly when both keys are
/// present (westerndigital: no filter 367 = REMOTE 3 + HYBRID 36 + ONSITE
/// 328, and a direct 100-row sample on 2026-08-22 confirmed both keys
/// present on 100/100 rows for that tenant) — but that measurement is about
/// the `locationType` **request** param, not payload-boolean completeness,
/// and says nothing about a tenant (or a future SmartRecruiters payload
/// change) that emits `remote` without `hybrid`. Any partial-absence shape
/// with no positive signal on either side stays `Unknown` (returns `None`).
fn smartrecruiters_work_type(remote: Option<bool>, hybrid: Option<bool>) -> Option<WorkType> {
    if hybrid == Some(true) {
        return Some(WorkType::Hybrid);
    }
    if remote == Some(true) {
        return Some(WorkType::Remote);
    }
    if remote == Some(false) && hybrid == Some(false) {
        return Some(WorkType::OnSite);
    }
    None
}

/// Whether `extra["remote"]` should be written `true`, given the CLASSIFIED
/// work type — never derived from the raw `remote` boolean directly. The two
/// source booleans are independent and both-true is representable (see
/// [`smartrecruiters_work_type`]'s doc), so a raw `remote:true` can resolve to
/// `Hybrid` once `hybrid` also wins the precedence; writing the raw boolean
/// would make `location_filter`'s "a board-flagged-remote posting can never
/// conflict with a place" short-circuit fire for a Hybrid job. Standalone so
/// the dual-write regression is unit-testable without a network round-trip.
fn smartrecruiters_is_declared_remote(work_type: Option<WorkType>) -> bool {
    work_type == Some(WorkType::Remote)
}

#[derive(Debug, Deserialize)]
struct Posting {
    id: String,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    uuid: Option<String>,
    name: String,
    location: Option<Location>,
    #[serde(rename = "releasedDate")]
    released_date: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResp {
    content: Vec<Posting>,
}

#[derive(Debug, Deserialize)]
struct Section {
    title: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobAd {
    sections: Option<std::collections::HashMap<String, Section>>,
}

#[derive(Debug, Deserialize)]
struct DetailResp {
    #[serde(rename = "jobAd")]
    job_ad: Option<JobAd>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}

/// Maximum number of company slugs processed per scrape call.
/// Each SmartRecruiters slug can produce one list request plus up to 100 detail
/// requests — an unbounded list from IPC would amplify outbound traffic severely.
const MAX_COMPANIES: usize = 20;

pub struct SmartRecruitersScraper;

#[async_trait]
impl Scraper for SmartRecruitersScraper {
    fn id(&self) -> &'static str {
        "smartrecruiters"
    }

    fn display_name(&self) -> &'static str {
        "SmartRecruiters"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    fn requires_company(&self) -> bool {
        true
    }

    /// SmartRecruiters is the only board in v1 that both validates a
    /// `locationType` param AND partitions its corpus exactly (no undeclared
    /// bucket) — see [`smartrecruiters_work_type`]. Live-verified: a bogus
    /// value returns an error object instead of silently ignoring it.
    fn supports_work_type(&self) -> bool {
        true
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Engine skips us when companies is empty; guard defensively anyway.
        if input.companies.is_empty() {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        // Dedupe (first-seen order), drop blanks, and cap to MAX_COMPANIES.
        // Each company can produce one list fetch + up to 100 detail fetches, so
        // an uncapped IPC payload would amplify traffic severely.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let company_count = companies.len();

        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (ci, company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = company.as_str();

            // SmartRecruiters supports a real `q` keyword param — pass it when set.
            let keyword = input.query.trim();
            // Upstream pass-through — the only board in v1 (see
            // `supports_work_type`). `work_type_spec()` (not the raw
            // `work_types` field) resolves the empty/absent-means-no-filter
            // invariant; `build_list_url` de-dupes again on its own (see its
            // doc comment) as defense-in-depth.
            let work_types = input.work_type_spec().unwrap_or_default();
            let list_url = build_list_url(company, keyword, &work_types);

            let list =
                match fetch_json::<ListResp>(&list_url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(l) => l,
                    Err(e) => {
                        // A fetch that failed because the run was cancelled is not
                        // a real board-level error.
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        // A non-2xx / schema-drift list response now records the
                        // failure so an all-slug 403 run returns Err (not a silent
                        // zero) via the all-fail check below.
                        log::warn!("[smartrecruiters] list fetch failed for '{}': {e}", company);
                        first_fetch_error.get_or_insert_with(|| e.to_string());
                        if let Some(ref on_progress) = ctx.on_progress {
                            on_progress((ci + 1) as f32 / company_count as f32);
                        }
                        continue;
                    }
                };
            successful_fetches += 1;
            let postings = list.content;

            let posting_count = postings.len();

            for (i, p) in postings.into_iter().enumerate() {
                if ctx.signal.is_cancelled() {
                    break;
                }

                let detail_url = format!(
                    "https://api.smartrecruiters.com/v1/companies/{}/postings/{}",
                    urlencoding::encode(company),
                    p.id
                );

                // Detail is best-effort: a failed detail fetch (404 / non-2xx /
                // schema drift) skips only this posting, never the whole board — the
                // list fetch already counted as a success. The progress emission +
                // politeness sleep still run so pacing and rate-limiting hold.
                let detail = match fetch_json::<DetailResp>(
                    &detail_url,
                    Default::default(),
                    ctx.signal.clone(),
                )
                .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        log::warn!(
                            "[smartrecruiters] detail fetch failed for posting {} ({detail_url}): {e}; skipping",
                            p.id
                        );
                        if let Some(ref on_progress) = ctx.on_progress {
                            on_progress(
                                (ci as f32 + (i + 1) as f32 / posting_count as f32)
                                    / company_count as f32,
                            );
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(
                            150 + (rand::random::<u64>() % 200),
                        ))
                        .await;
                        continue;
                    }
                };

                let sections = detail.job_ad.and_then(|ja| ja.sections).unwrap_or_default();

                // Sort by key for deterministic section order — HashMap iteration
                // order is non-deterministic and would produce unstable descriptions.
                let mut sections_sorted: Vec<(&String, &Section)> = sections.iter().collect();
                sections_sorted.sort_by_key(|(k, _)| k.as_str());

                let description = sections_sorted
                    .iter()
                    .map(|(_, s)| {
                        format!(
                            "{}\n{}",
                            s.title.as_deref().unwrap_or(""),
                            strip_html(s.text.as_deref().unwrap_or(""))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
                    .trim()
                    .to_string();

                let location = vec![
                    p.location.as_ref().and_then(|l| l.city.clone()),
                    p.location.as_ref().and_then(|l| l.country.clone()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");

                let posted_at = p
                    .released_date
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.timestamp_millis());

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), p.id),
                    external_id: Some(p.id.clone()),
                    title: p.name,
                    company: company.to_string(),
                    location: if location.is_empty() {
                        None
                    } else {
                        Some(location)
                    },
                    url: format!(
                        "https://jobs.smartrecruiters.com/{}/{}",
                        urlencoding::encode(company),
                        p.id
                    ),
                    source: self.id().to_string(),
                    description: if description.is_empty() {
                        None
                    } else {
                        Some(description)
                    },
                    requirements: None,
                    posted_at,
                    captured_at: now,
                    extra: {
                        let mut map = std::collections::HashMap::new();
                        let remote_flag = p.location.as_ref().and_then(|l| l.remote);
                        let hybrid_flag = p.location.as_ref().and_then(|l| l.hybrid);
                        let work_type = smartrecruiters_work_type(remote_flag, hybrid_flag);
                        if smartrecruiters_is_declared_remote(work_type) {
                            map.insert("remote".to_string(), serde_json::json!(true));
                        }
                        if let Some(wt) = work_type {
                            map.insert("workType".to_string(), serde_json::json!(wt));
                        }
                        map
                    },
                };

                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }

                out.push(posting);

                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress(
                        (ci as f32 + (i + 1) as f32 / posting_count as f32) / company_count as f32,
                    );
                }

                // Small jitter between per-listing detail fetches (rate-limit
                // politeness; mirrors the arbeitsagentur per-page delay pattern).
                tokio::time::sleep(std::time::Duration::from_millis(
                    150 + (rand::random::<u64>() % 200),
                ))
                .await;
            }

            // Emit company-level progress after exhausting all postings for this slug.
            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((ci + 1) as f32 / company_count as f32);
            }
        }

        // Distinguishes an all-slug 403 run from a genuine zero result (detail-only
        // failures don't count — the list fetch already counted as a success); see
        // `ats_all_fetches_failed` for the decision.
        if let Some(message) =
            ats_all_fetches_failed(self.id(), successful_fetches, &first_fetch_error)
        {
            return Err(anyhow::anyhow!(message));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
