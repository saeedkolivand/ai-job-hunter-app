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

/// Map SmartRecruiters' `location.remote` / `location.hybrid` booleans to a
/// declared work type, precedence **hybrid > remote > on-site**.
///
/// **Board-specific rule, proven by live measurement, not the generic
/// all-false→Unknown policy used elsewhere:** `locationType` partitions this
/// board EXACTLY (westerndigital: no filter 367 = REMOTE 3 + HYBRID 36 +
/// ONSITE 328) — there is no undeclared bucket, so `remote:false &&
/// hybrid:false` genuinely means on-site here. Only writes a value when at
/// least one of the two booleans is present in the payload; both absent means
/// the field itself was missing, which correctly stays `Unknown`.
fn smartrecruiters_work_type(remote: Option<bool>, hybrid: Option<bool>) -> Option<WorkType> {
    if remote.is_none() && hybrid.is_none() {
        return None;
    }
    if hybrid == Some(true) {
        Some(WorkType::Hybrid)
    } else if remote == Some(true) {
        Some(WorkType::Remote)
    } else {
        Some(WorkType::OnSite)
    }
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
            let mut list_url = if keyword.is_empty() {
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
            // Upstream pass-through — the only board in v1 (see
            // `supports_work_type`). Repeatable param, one occurrence per
            // requested type. `work_type_spec()` (not the raw `work_types`
            // field) both resolves the empty/absent-means-no-filter invariant
            // and dedupes — CWE-770 defense-in-depth against a generous
            // multi-select fanning out into an unbounded `&locationType=` tail.
            if let Some(work_types) = input.work_type_spec() {
                for wt in &work_types {
                    list_url.push_str("&locationType=");
                    list_url.push_str(smartrecruiters_location_type_param(*wt));
                }
            }

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
                        if let Some(remote) = remote_flag {
                            map.insert("remote".to_string(), serde_json::json!(remote));
                        }
                        if let Some(wt) = smartrecruiters_work_type(remote_flag, hybrid_flag) {
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
