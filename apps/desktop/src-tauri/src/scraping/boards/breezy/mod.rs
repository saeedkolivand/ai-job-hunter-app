//! Breezy HR — public per-company postings JSON
//!
//! Endpoint: `https://{slug}.breezy.hr/json`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
//!
//! Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/breezy.mjs`.
//! Live-verified 2026-07-11 against slug `breezy` (3 jobs). DRIFT FOUND + FIXED:
//! `location.state` is an OBJECT (`{id, name}`) on the live API, not the bare
//! string the career-ops port assumed — the old atomic `Vec<BzPosting>`
//! deserialize failed every real tenant's whole payload (silent zero). Now
//! `BzStateField` tolerates both shapes and rows deserialize per-row via
//! `rows_to_jobs` (mirrors Rippling/Workable), so one drifted row can't zero the
//! board. Confirmed present: `name`, `url`, `published_date` (RFC3339),
//! `location{name, city, state{name}, country{name}, is_remote}`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::{
    ats_finish_search, ats_partial_note, is_https_url, is_valid_dns_label_slug, normalize_companies,
};
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "breezy";

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Parse a `published_date` value that may be a full RFC3339 timestamp or a
/// bare `YYYY-MM-DD` date. Returns `None` on any unparseable value.
fn parse_breezy_date(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
}

#[derive(Debug, Deserialize)]
struct BzCountry {
    name: Option<String>,
}

/// Breezy's `location.state` is an OBJECT (`{id, name}`) on the live API
/// (verified 2026-07-11) but a bare string on some tenants/fixtures — accept
/// both so one shape doesn't fail the row. `Text` (a JSON string) and the object
/// form are unambiguous under serde's untagged matching (a string can only be
/// `Text`; an object can only be `Named`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BzStateField {
    Text(String),
    Named { name: Option<String> },
}

impl BzStateField {
    fn into_name(self) -> Option<String> {
        match self {
            BzStateField::Text(s) => Some(s),
            BzStateField::Named { name } => name,
        }
    }
}

#[derive(Debug, Deserialize)]
struct BzLocation {
    name: Option<String>,
    city: Option<String>,
    state: Option<BzStateField>,
    country: Option<BzCountry>,
    #[serde(rename = "is_remote")]
    is_remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BzPosting {
    name: Option<String>,
    url: Option<String>,
    #[serde(rename = "published_date")]
    published_date: Option<String>,
    location: Option<BzLocation>,
}

/// Deserialize each posting row independently, dropping (with a debug log) any
/// row that fails to type-check — e.g. a future field-shape drift on one row.
/// Without this, an atomic `Vec<BzPosting>` deserialize would fail the WHOLE
/// company on a single malformed row (silent zero-jobs) — which is exactly how
/// the live `location.state`-as-object shape broke every real tenant before this
/// PR. Mirrors Rippling's/Workable's `rows_to_jobs`.
pub(crate) fn rows_to_jobs(values: Vec<serde_json::Value>) -> Vec<BzPosting> {
    let total = values.len();
    let jobs: Vec<BzPosting> = values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<BzPosting>(v) {
            Ok(job) => Some(job),
            Err(e) => {
                log::debug!("[breezy] skipping malformed row: {e}");
                None
            }
        })
        .collect();
    let skipped = total - jobs.len();
    if skipped > 0 {
        log::warn!("[breezy] skipped {skipped}/{total} malformed rows");
    }
    jobs
}

/// Map a parsed Breezy response into postings for one company, plus a FORMAT
/// drop count for the `rows-dropped:<n>` partial-visibility note (trust-H item
/// 3; CodeRabbit follow-up on PR #604). **The boundary:** a row counts as a
/// format drop when it deserialized fine (passed [`rows_to_jobs`]) but is
/// unusable as a posting — a missing/blank title, or a missing/unparseable
/// url — the same "this row's shape looks wrong" signal `rows_to_jobs`
/// reports one level earlier, so the note stays about DRIFT. Duplicate-url
/// drops are DELIBERATELY EXCLUDED from the count: two rows sharing one apply
/// link is normal Breezy multi-listing hygiene (e.g. a posting cross-listed
/// under two departments), not evidence the response shape changed — counting
/// it would make `"rows unreadable — board format may have changed"` fire on
/// a perfectly healthy response. Standalone (no `&self`) so it is
/// unit-testable against a JSON fixture.
///
/// The response has no stable job id, so the (deduped) posting URL doubles as
/// the id — same precedent as the We Work Remotely RSS `<guid>` permalink.
pub(crate) fn parse_breezy_response(
    postings: Vec<BzPosting>,
    company: &str,
    now: i64,
) -> (Vec<JobPosting>, usize) {
    let mut seen_urls = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut format_drops = 0usize;

    for p in postings {
        let title = p.name.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            format_drops += 1;
            continue;
        }

        let url = match p.url.as_deref().map(str::trim) {
            Some(u) if is_https_url(u) => u.to_string(),
            _ => {
                format_drops += 1;
                continue;
            }
        };
        if !seen_urls.insert(url.clone()) {
            // Duplicate url — normal hygiene, never a format drop (see the doc
            // comment above).
            continue;
        }

        let posted_at = p.published_date.as_deref().and_then(parse_breezy_date);

        let location = p.location.and_then(|l| {
            let is_remote = l.is_remote.unwrap_or(false);
            let mut base = match l.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(name) => name.to_string(),
                None => {
                    let parts: Vec<String> = [
                        l.city,
                        l.state.and_then(BzStateField::into_name),
                        l.country.and_then(|c| c.name),
                    ]
                    .into_iter()
                    .flatten()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                    parts.join(", ")
                }
            };
            if is_remote && !base.to_lowercase().contains("remote") {
                if base.is_empty() {
                    base = "Remote".to_string();
                } else {
                    base.push_str(", Remote");
                }
            }
            (!base.is_empty()).then_some(base)
        });

        out.push(JobPosting {
            id: format!("{BOARD_ID}:{url}"),
            external_id: Some(url.clone()),
            title,
            company: company.to_string(),
            location,
            url,
            source: BOARD_ID.to_string(),
            description: None,
            requirements: None,
            posted_at,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    (out, format_drops)
}

pub struct BreezyScraper;

#[async_trait]
impl Scraper for BreezyScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Breezy HR"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    fn requires_company(&self) -> bool {
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

        // Dedupe (first-seen order), drop blanks, and cap to MAX_COMPANIES so a
        // large IPC payload cannot fan out unbounded requests to Breezy.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut rejected_slugs = 0usize;
        let mut rows_dropped = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, raw_company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = raw_company.to_lowercase();

            // Guard: reject slugs that are not valid single DNS hostname labels.
            // A slug like `127.0.0.1:8443/foo` would change the URL authority
            // and redirect the fetch away from Breezy (SSRF).
            if !is_valid_dns_label_slug(&company) {
                rejected_slugs += 1;
                log::warn!("[breezy] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url = format!("https://{company}.breezy.hr/json");

            // Fetch as raw rows so `rows_to_jobs` can drop a single drifted row
            // instead of failing the whole company (see the module doc: the live
            // `location.state` object shape used to zero every tenant here).
            let data = match fetch_json::<Vec<serde_json::Value>>(
                &url,
                Default::default(),
                ctx.signal.clone(),
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    // Check cancellation first: a fetch that failed because
                    // the run was cancelled is not a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[breezy] fetch failed for '{}': {e}", company);
                    first_fetch_error.get_or_insert_with(|| e.to_string());
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            // A non-2xx response is now an `Err` above (which records
            // `first_fetch_error`). A 200 where EVERY row fails to deserialize is
            // schema drift too — the exact failure class the `location.state`
            // object drift caused before `rows_to_jobs` existed — so treat that
            // as a fetch failure rather than a silent success-with-zero-jobs
            // (round-2 review finding).
            let raw_row_count = data.len();
            let postings = rows_to_jobs(data);
            if raw_row_count > 0 && postings.is_empty() {
                log::warn!(
                    "[breezy] all {raw_row_count} rows failed to parse for '{}'; treating as a fetch failure",
                    company
                );
                first_fetch_error.get_or_insert_with(|| {
                    format!("all {raw_row_count} rows failed to parse — response shape may have changed")
                });
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }
            successful_fetches += 1;
            // SOME (not all) rows dropped by per-row parse (rows_to_jobs
            // deserialize failures) → tallied here; `parse_breezy_response`'s
            // own FORMAT-relevant drops (empty title / bad url — NOT
            // duplicate-url drops, see its doc comment) are folded in below so
            // `rows-dropped:<n>` doesn't undercount (CodeRabbit, PR #604).
            rows_dropped += raw_row_count - postings.len();

            let (parsed, format_drops) = parse_breezy_response(postings, &company, now);
            rows_dropped += format_drops;

            for posting in parsed {
                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }
                out.push(posting);
            }

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        // trust-H item 3: surface a partial-visibility anomaly (some slugs
        // rejected / some rows dropped, while others succeeded) as ONE
        // informational note (PR D grammar; `slugs-invalid` wins over
        // `rows-dropped`). Gated on non-cancellation — a benign interruption
        // reports nothing (mirrors `ats_finish_search`).
        if !ctx.signal.is_cancelled() {
            if let Some(note) = ats_partial_note(successful_fetches, rejected_slugs, rows_dropped) {
                ctx.report_note(note);
            }
        }

        // See `ats_finish_search`: cancellation wins over a synthesized
        // all-fetches-failed/all-slugs-invalid board error.
        ats_finish_search(
            &ctx.signal,
            out,
            self.id(),
            successful_fetches,
            rejected_slugs,
            &first_fetch_error,
        )
    }
}

#[cfg(test)]
mod test;
