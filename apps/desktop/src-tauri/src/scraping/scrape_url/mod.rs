//! URL → JobPosting resolver.
//!
//! Given an arbitrary job posting URL, return a `JobPosting` by:
//! 1. Recognising known board URL patterns and hitting their public API
//!    (Greenhouse, Lever, Ashby, LinkedIn, Workday, SmartRecruiters, Personio).
//! 2. If no named board matched the original URL, following the redirect chain
//!    (via the IP-guarded client) to the FINAL URL, then re-dispatching the
//!    named-board handlers on that URL — so an aggregator click-tracker
//!    (e.g. Adzuna `redirect_url`) that lands on a Greenhouse/Lever/… posting
//!    yields the full board-API text rather than weak generic-HTML extraction.
//! 3. Falling back to a generic HTML parse on the final URL/body.

use anyhow::Result;

use crate::scraping::types::JobPosting;

mod ashby;
mod greenhouse;
mod html_fallback;
mod lever;
mod linkedin;
mod personio;
mod smartrecruiters;
mod workday;

use ashby::try_ashby;
use greenhouse::try_greenhouse;
use lever::try_lever;
use linkedin::try_linkedin;
use personio::try_personio;
use smartrecruiters::try_smartrecruiters;
use workday::try_workday;

pub(crate) use html_fallback::job_root_generic_html;
pub use html_fallback::parse_from_html;
pub(crate) use personio::personio_company_from_url;

/// Byte cap for a generic-HTML body read here — the same 8 MB the board fetch
/// path applies. A job page that legitimately exceeds this is not something the
/// generic extractor could make sense of anyway.
const SCRAPE_URL_MAX_BYTES: usize = 8 * 1024 * 1024;

/// Try every named-board handler on `url` in order; return the first match.
/// Returns `Ok(None)` when no board recognises the URL (without making any
/// network request beyond the board-pattern parse — each handler does an early
/// return on a non-matching host).
async fn try_named_boards(url: &str) -> Result<Option<JobPosting>> {
    if let Some(p) = try_greenhouse(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_lever(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_ashby(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_linkedin(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_workday(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_smartrecruiters(url).await? {
        return Ok(Some(p));
    }
    if let Some(p) = try_personio(url).await? {
        return Ok(Some(p));
    }
    Ok(None)
}

/// Resolve `url` to a [`JobPosting`], with a trust assessment always attached
/// (see [`crate::scraping::trust::attach`]) — the single point every caller
/// (the `scrape_url`/`scrape_resolve_url` commands and the extension-bridge
/// import) shares, so none of them need to compute it themselves.
pub async fn resolve(url: &str) -> Result<Option<JobPosting>> {
    let posting = resolve_uncached(url).await?;
    Ok(posting.map(|mut p| {
        crate::scraping::trust::attach(&mut p);
        p
    }))
}

async fn resolve_uncached(url: &str) -> Result<Option<JobPosting>> {
    // Pass 1: try named boards on the original URL (fast path — no redirect
    // follow needed when the caller already holds a direct board URL).
    if let Some(posting) = try_named_boards(url).await? {
        return Ok(Some(posting));
    }

    // Pass 2: follow the redirect chain to the FINAL URL through the IP-guarded
    // client (closes SSRF / DNS-rebinding TOCTOU). Each hop is re-validated.
    // Cap: 2 hops (aggregator click-tracker → real posting is typically 1 hop;
    // 2 covers a CDN bounce). Callers (scrape_resolve_url command and
    // extension_bridge handle_import) are responsible for acquiring a limiter
    // slot before calling resolve() — this fn is limiter-agnostic.
    let res = match crate::net::http::get_guarded_following_redirects(url, 2).await {
        Ok(r) => r,
        // Redirect chain failed (DNS, SSRF, network) → keep snippet, no panic.
        Err(_) => return Ok(None),
    };

    // 429 / login-wall / any non-2xx (e.g. Adzuna click-tracker error) →
    // return None so the renderer keeps its existing snippet.
    if !res.status().is_success() {
        return Ok(None);
    }

    // `res.url()` is the URL of the last-hop request — it equals the final
    // destination because get_guarded uses redirect::Policy::none() on every
    // hop, so each response is exactly the request we sent (no silent redirect
    // following inside reqwest that would shift the URL under us).
    let final_url = res.url().to_string();

    // Pass 3: re-dispatch named boards on the FINAL URL. This is the key step
    // for aggregator redirects: an Adzuna `redirect_url` → Greenhouse page will
    // now hit the Greenhouse API handler and return full board-API text.
    // Skip if the URL didn't change (no redirect occurred) — we already tried.
    if final_url != url {
        if let Some(posting) = try_named_boards(&final_url).await? {
            return Ok(Some(posting));
        }
    }

    // Pass 4: generic HTML fallback on the already-fetched body — no second
    // fetch. The body was fetched through the guarded client so the host is
    // already validated; parse it directly.
    //
    // Read it through the SAME byte cap the board path uses. This URL is
    // attacker-influenced (that is why it goes through `get_guarded*`), and
    // `Response::text()` buffers the whole body — a hostile host could stream an
    // arbitrarily large one and drive us into OOM. The 20s timeout bounds how
    // LONG we read, not how MUCH. Going through `fetch_text` instead is not an
    // option here: it uses the shared client and would drop the SSRF guard.
    let html = match crate::net::http::read_text_capped(res, SCRAPE_URL_MAX_BYTES).await {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    Ok(parse_from_html(&final_url, &html))
}

/// Map a board's search / SPA "list + detail pane" view URL (where the SELECTED
/// job's id lives in a query param) to the canonical single-job URL. Returns
/// `None` when the URL is already a direct job page or the host is unrecognized —
/// the caller then uses the URL as-is. This is the single, centralized place that
/// knows "which job is selected in this SPA view"; every board plugs in via one
/// match arm. Ids are validated before being interpolated into a URL we will
/// later fetch (defense-in-depth alongside the import path's SSRF guard).
pub fn canonical_job_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let query = |key: &str| {
        u.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    // LinkedIn: /jobs/search|collections/...?currentJobId=<id> → /jobs/view/<id>.
    // Numeric id only. Skip when already a direct /jobs/view/ page.
    if host == "linkedin.com" || host == "www.linkedin.com" || host.ends_with(".linkedin.com") {
        if !u.path().contains("/jobs/view/") {
            if let Some(id) = query("currentJobId") {
                if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                    return Some(format!("https://www.linkedin.com/jobs/view/{id}"));
                }
            }
        }
        return None;
    }

    // Indeed (incl. country TLDs like de.indeed.com): ?vjk=<id> → /viewjob?jk=<id>.
    // Alphanumeric id only. Skip when already a /viewjob page.
    if host == "indeed.com" || host.ends_with(".indeed.com") {
        if !u.path().contains("/viewjob") {
            if let Some(id) = query("vjk") {
                if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    return Some(format!("https://{host}/viewjob?jk={id}"));
                }
            }
        }
        return None;
    }

    // Xing (live-probed, PR 7): as observed in public/no-login sessions, selecting
    // a job in the list navigates straight to the path-canonical URL — clicking a
    // job title is a full same-tab navigation to `xing.com/jobs/<slug>-<id>`, never
    // a shell URL with the selection only in a query param. So there is nothing for
    // this function to rewrite; the per-visit tracking param Xing appends (`?ijt=`)
    // is dropped whole by `applications::normalize_job_url`'s `retain_identifying_
    // params` step, which consults `identifying_query_params(host)` for what to
    // keep — Xing has no entry there, so the whole query is dropped. See
    // `canonical_xing_*` tests below for the pinned evidence.
    //
    // TODO(import): StepStone — the same "nothing to rewrite" finding held for the
    // public/no-login list flow (a job title opens the canonical detail URL, id in
    // the path, in a new tab — `stepstone.de/stellenangebote--<slug>--<id>-inline.
    // html`; its tracking param `?rltr=` is dropped the same way as Xing's `?ijt=`,
    // see `canonical_stepstone_*` tests below). But the site also has a login-gated
    // "inline preview" / split-view mode — a signup modal intercepted the
    // card-body click during the live probe, so that mode was (correctly) never
    // explored, and this resolver's only real caller is the extension import path
    // on the user's authenticated tab. That mode may carry the selected job in a
    // query param instead. Reopen if authenticated imports are observed resolving
    // a list shell rather than the selected job.
    //
    // TODO(import): Glassdoor (jobListingId/jl) — still needs a real captured URL;
    // glassdoor.com/.de returned a Cloudflare "Just a moment…" challenge page for
    // this session (homepage + search, both TLDs), blocking live verification.
    // Tracked as a follow-up.
    None
}

// `(board, id)` identity extraction (issue #1166) — a sibling of
// `canonical_job_url` above, split into its own file purely to keep this
// module under the R8 LOC hard cap (`docs/architecture-rules.md`); the
// per-board url knowledge stays in this SAME directory, re-exported below so
// every caller keeps writing `scrape_url::job_identity` unchanged.
mod identity;
pub use identity::job_identity;

/// LinkedIn (and similar pages) render "Show more" / "Show less" toggle buttons
/// right after the description markup; strip those trailing labels.
static SHOW_MORE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(\s*(show more|show less))+\s*$").unwrap());

fn clean_description(text: &str) -> String {
    SHOW_MORE_RE.replace(text, "").trim().to_string()
}

mod generic;
pub(crate) use generic::embeds_ats_board;

#[cfg(test)]
mod tests;
