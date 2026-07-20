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

use crate::scraping::types::JobPosting;
use anyhow::Result;
use scraper::{Html, Selector};
use std::collections::HashMap;

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
    let html = match res.text().await {
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

/// LinkedIn (and similar pages) render "Show more" / "Show less" toggle buttons
/// right after the description markup; strip those trailing labels.
static SHOW_MORE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(\s*(show more|show less))+\s*$").unwrap());

fn clean_description(text: &str) -> String {
    SHOW_MORE_RE.replace(text, "").trim().to_string()
}

// ── Greenhouse ──────────────────────────────────────────────────────────────
//
// URL: https://boards.greenhouse.io/<company>/jobs/<job_id>
// API: https://boards-api.greenhouse.io/v1/boards/<company>/jobs/<job_id>

async fn try_greenhouse(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_greenhouse_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://boards-api.greenhouse.io/v1/boards/{}/jobs/{}",
        urlencoding::encode(&company),
        urlencoding::encode(&job_id),
    );
    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let title = v
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("location")
        .and_then(|l| l.get("name"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("content")
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::html_to_markdown);
    let abs_url = v
        .get("absolute_url")
        .and_then(|s| s.as_str())
        .unwrap_or(url)
        .to_string();
    let updated_at = v
        .get("updated_at")
        .and_then(|s| s.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis());

    Ok(Some(JobPosting {
        id: format!("greenhouse:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: abs_url,
        source: "greenhouse".to_string(),
        description,
        requirements: None,
        posted_at: updated_at,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

fn parse_greenhouse_url(url: &str) -> Option<(String, String)> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    if !host.ends_with("greenhouse.io") {
        return None;
    }
    let segments: Vec<&str> = u.path_segments()?.collect();
    // Patterns:
    //  /<company>/jobs/<id>            (boards.greenhouse.io)
    //  /embed/job_app?for=<company>&token=<id>
    if segments.len() >= 3 && segments[1] == "jobs" {
        return Some((segments[0].to_string(), segments[2].to_string()));
    }
    if segments.first() == Some(&"embed") {
        let mut company = None;
        let mut token = None;
        for (k, v) in u.query_pairs() {
            match k.as_ref() {
                "for" => company = Some(v.into_owned()),
                "token" => token = Some(v.into_owned()),
                _ => {}
            }
        }
        if let (Some(c), Some(t)) = (company, token) {
            return Some((c, t));
        }
    }
    None
}

// ── Lever ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.lever.co/<company>/<id>
// API: https://api.lever.co/v0/postings/<company>/<id>

async fn try_lever(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_lever_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://api.lever.co/v0/postings/{}/{}",
        urlencoding::encode(&company),
        urlencoding::encode(&job_id),
    );
    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let title = v
        .get("text")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("categories")
        .and_then(|c| c.get("location"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("descriptionPlain")
        .and_then(|s| s.as_str())
        .map(str::to_string)
        .or_else(|| {
            v.get("description")
                .and_then(|s| s.as_str())
                .map(crate::scraping::http::html_to_markdown)
        });
    let abs_url = v
        .get("hostedUrl")
        .and_then(|s| s.as_str())
        .unwrap_or(url)
        .to_string();
    let created_at = v.get("createdAt").and_then(|n| n.as_i64());

    Ok(Some(JobPosting {
        id: format!("lever:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: abs_url,
        source: "lever".to_string(),
        description,
        requirements: None,
        posted_at: created_at,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

fn parse_lever_url(url: &str) -> Option<(String, String)> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    if !host.ends_with("lever.co") {
        return None;
    }
    let segments: Vec<&str> = u.path_segments()?.collect();
    if segments.len() >= 2 {
        return Some((segments[0].to_string(), segments[1].to_string()));
    }
    None
}

// ── Ashby ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.ashbyhq.com/<company>/<id>

async fn try_ashby(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host.ends_with("ashbyhq.com") {
        return Ok(None);
    }
    let segments: Vec<&str> = match u.path_segments().map(|s| s.collect()) {
        Some(v) => v,
        None => return Ok(None),
    };
    if segments.len() < 2 {
        return Ok(None);
    }
    let company = segments[0].to_string();
    let job_id = segments[1].to_string();

    // Public GraphQL endpoint for a single posting.
    let body = serde_json::json!({
        "operationName": "ApiJobPosting",
        "variables": { "organizationHostedJobsPageName": company, "jobPostingId": job_id },
        "query": "query ApiJobPosting($organizationHostedJobsPageName: String!, $jobPostingId: String!) { jobPosting(organizationHostedJobsPageName: $organizationHostedJobsPageName, jobPostingId: $jobPostingId) { title locationName departmentName descriptionPlain } }"
    });
    let client = crate::net::http::shared();
    let res = client
        .post("https://jobs.ashbyhq.com/api/non-user-graphql?op=ApiJobPosting")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let p = v.get("data").and_then(|d| d.get("jobPosting"));
    let p = match p {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };
    let title = p
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = p
        .get("locationName")
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = p
        .get("descriptionPlain")
        .and_then(|s| s.as_str())
        .map(str::to_string);

    Ok(Some(JobPosting {
        id: format!("ashby:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: url.to_string(),
        source: "ashby".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── Generic HTML fallback ───────────────────────────────────────────────────

/// Build a `JobPosting` from already-fetched HTML.  The extension-bridge
/// **Scan mode** supplies the authenticated DOM (a logged-in board page the
/// desktop's anonymous fetch can't see), so it reuses this exact parse path
/// instead of re-fetching.  [`resolve`] calls this after it has followed any
/// redirect chain and exhausted the named-board re-dispatch.
///
/// Prefers JSON-LD `JobPosting` fields (title/description/location/company) when
/// the page ships them — structured data always wins over any DOM guess. Below
/// that, the base pass is the generic `<title>`/`<h1>` + meta description parse,
/// with the extension's `[data-ajh-job-root="true"]` hint (when present)
/// overriding title/description FIELD BY FIELD rather than wholesale — see the
/// per-field merge note below. `parse_generic_company` supplies the employer
/// name. Always returns `Some` for a successfully-parsed document — the title
/// may be an empty string (e.g. a page with only a meta description) so the
/// description-on-demand flow still surfaces the page. (The fetch half
/// short-circuits earlier on a non-success status / rejected host.)
pub fn parse_from_html(url: &str, html: &str) -> Option<JobPosting> {
    // Base: whole-document <title>/first-<h1> + meta description (today's floor).
    let (mut title, mut description) = parse_generic_html(html);

    // Prefer the extension's `[data-ajh-job-root="true"]` hint (Scan-mode's
    // best-effort "main job content" mark — see `markLikelyJobNode` in
    // `apps/extension/src/content.ts`) over the base pass above, but FIELD BY
    // FIELD rather than wholesale: override `title` only when the hinted
    // subtree found a non-empty one, override `description` only when it found
    // one. A wholesale swap would be wrong on a thin-body page (e.g. an ATS
    // embeds the real description in an iframe and the hinted node is just an
    // `<h1>`) — it would clobber a good document-level meta description with
    // nothing useful. A mis-marked/hostile hint (wrong element, script/
    // whitespace-only) yields (empty, `None`) for both fields, so it overrides
    // neither — the per-field fallback is the guarantee that a bad hint can
    // never make results worse than before.
    //
    // `hint_title_used` tracks whether the hint actually supplied a usable
    // title — the signal the last-resort fallback below uses to tell "thin
    // hint" (real signal, just no body) apart from "hostile/mis-marked hint"
    // (no signal at all, treat as if there were no hint).
    let mut hint_title_used = false;
    if let Some((hint_title, hint_description)) = job_root_generic_html(html) {
        if !hint_title.is_empty() {
            title = hint_title;
            hint_title_used = true;
        }
        if hint_description.is_some() {
            description = hint_description;
        }
    }
    let mut location = None;

    // JSON-LD `JobPosting` is the richest source when present — let it override
    // the generic title/description and supply a location the meta tags lack.
    if let Some(jl) = json_ld_job_posting(html) {
        if !jl.title.is_empty() {
            title = jl.title;
        }
        if jl.description.is_some() {
            description = jl.description;
        }
        if jl.location.is_some() {
            location = jl.location;
        }
    }

    // `__NEXT_DATA__` fallback: fill ONLY fields JSON-LD left missing (don't
    // clobber good JSON-LD values).
    if title.is_empty() || description.is_none() {
        if let Some(nd) = next_data_job(html) {
            if title.is_empty() && !nd.title.is_empty() {
                title = nd.title;
            }
            if description.is_none() {
                description = nd.description;
            }
            if location.is_none() {
                location = nd.location;
            }
        }
    }

    // Whole-document last-resort description — SKIPPED when the hint already
    // supplied a real title. `job_root_generic_html` scopes its own
    // description search to that same hinted subtree, so if it honestly found
    // no body text there (a title-only hint — e.g. the real description
    // renders client-side in an ATS iframe the outerHTML capture can't see),
    // escalating to a whole-DOCUMENT guess risks landing on an unrelated block
    // (e.g. a bigger related-jobs sidebar) instead of just admitting there's
    // no description to show. A hint that yielded NOTHING (no title either —
    // hostile/mis-marked) carries no signal, so the whole-document heuristic
    // chain still runs exactly as if there were no hint at all.
    //
    // Within that chain, `readability_content_text` (a real Mozilla-Readability
    // port) runs FIRST — it scores/prunes nav, footer, and boilerplate rather
    // than just picking the largest `main`/`article` block, so it's the
    // higher-precision guess on a page with no JSON-LD. `main_content_text`'s
    // largest-block guess stays as the final fallback for when readability's
    // own pre-check (`is_probably_readable`) or `parse()` comes back empty.
    if description.is_none() && !hint_title_used {
        description = readability_content_text(url, html).or_else(|| main_content_text(html));
    }

    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    // Prefer a real employer name (JSON-LD / og:site_name) over the bare host.
    let company = parse_generic_company(html).unwrap_or(host);

    Some(JobPosting {
        id: format!("url:{}", url),
        external_id: None,
        title,
        company,
        location,
        url: url.to_string(),
        source: "url".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    })
}

/// Extraction from the extension's best-effort `[data-ajh-job-root="true"]`
/// hint (pinned to the contract value — `markLikelyJobNode` in
/// `apps/extension/src/content.ts` only ever sets `"true"`). Title is the first
/// `h1` inside the hinted subtree; description is the subtree's own rendered
/// text, EXCLUDING that title heading's own markup (see the `JOB_ROOT_TITLE_RE`
/// strip below) so a hinted node that is just a heading — the real body
/// rendered elsewhere, e.g. an ATS iframe the outerHTML capture can't see —
/// yields no description at all rather than a title-redundant stub. Scoping to
/// the hinted node — rather than re-running the largest-`main`/`article`-block
/// guess ([`main_content_text`]) over the whole document — lets a page with
/// several `main`/`article`-shaped blocks (e.g. a related-jobs sidebar bigger
/// than the actual posting) resolve to the right one. Returns `None` when no
/// hinted node exists in the document at all, so the no-hint path (including
/// every server-fetch resolve where the hint can never be present) is
/// untouched. The caller (`parse_from_html`) applies title and description
/// independently, so either field alone may come back empty/`None`.
// ponytail: single-node lookup + a couple of child selectors, no depth cap
// needed (unlike the JSON-LD/NEXT_DATA walks) — scoped to whatever the
// extension already marked.
fn job_root_generic_html(html: &str) -> Option<(String, Option<String>)> {
    // Cheap substring check before the full-document reparse below: every
    // server-fetch resolve call hits this with no hint attribute present at
    // all, so skip `Html::parse_document` entirely on that (overwhelmingly
    // common) path.
    if !html.contains("data-ajh-job-root") {
        return None;
    }

    let doc = Html::parse_document(html);
    let root_sel = Selector::parse(r#"[data-ajh-job-root="true"]"#).ok()?;
    // First-in-DOM-order match is the intended tie-break if a page ever
    // contains several hinted nodes.
    let root = doc.select(&root_sel).next()?;

    // Unlike this crate's own `html_to_text`, `html_to_markdown`'s `htmd` backend
    // does not treat `<script>`/`<style>` as non-rendered — it leaks their raw
    // contents into the output. A hostile/mis-marked hint node containing only a
    // tracking script must not read as a "real" title or description, so strip
    // them first — before extracting EITHER field, not just the description.
    let inner_html = root.inner_html();
    let cleaned = JOB_ROOT_SCRIPT_STYLE_RE.replace_all(&inner_html, " ");
    let cleaned_doc = Html::parse_fragment(&cleaned);

    let title_sel = Selector::parse("h1").ok()?;
    let title = cleaned_doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    // Exclude ONLY the title heading (the first <h1>) from the description
    // source: an h1-only hinted subtree must not turn its own title into a
    // redundant description stub that would then clobber a real document-level
    // meta description in the caller's per-field merge. `replacen(.., 1, ..)`
    // rather than `replace_all` — a later `<h1>` is a legitimate section
    // heading (e.g. "Responsibilities") and must survive into the description.
    let body_html = JOB_ROOT_TITLE_RE.replacen(&cleaned, 1, " ");
    let description = crate::scraping::http::html_to_markdown(&body_html);
    let description = (!description.trim().is_empty()).then_some(description);

    Some((title, description))
}

static JOB_ROOT_SCRIPT_STYLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?is)<(script|style)[\s\S]*?</(script|style)>").unwrap()
    });

/// Matches an `<h1>` element in the hinted subtree's (already script/style-
/// cleaned) HTML; the caller only ever strips the FIRST match (`replacen`, not
/// `replace_all`) — see the "exclude the title" note on [`job_root_generic_html`]
/// above. A later `<h1>` is a legitimate section heading, not the title.
static JOB_ROOT_TITLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)<h1[\s\S]*?</h1>").unwrap());

/// The subset of JSON-LD `JobPosting` fields the generic parse path consumes.
struct JsonLdJob {
    title: String,
    description: Option<String>,
    location: Option<String>,
}

/// Format one JSON-LD `PostalAddress`-shaped node to a display string.
/// Locality-first (`"City, Region"` / `"City"` / `"Region"`); `addressCountry`
/// is a fallback ONLY when both locality and region are absent.
// ponytail: country used only as a fallback when no locality/region, to preserve
// the locality-first golden ("Berlin, BE" stays "Berlin, BE", not ", BE, DE").
fn fmt_address(addr: &serde_json::Value) -> Option<String> {
    let locality = addr.get("addressLocality").and_then(|s| s.as_str());
    let region = addr.get("addressRegion").and_then(|s| s.as_str());
    match (locality, region) {
        (Some(c), Some(r)) => Some(format!("{c}, {r}")),
        (Some(c), None) => Some(c.to_string()),
        (None, Some(r)) => Some(r.to_string()),
        (None, None) => addr
            .get("addressCountry")
            .and_then(|s| s.as_str())
            .map(str::to_string),
    }
}

/// Pull a display location from a `JobPosting`'s `jobLocation`, which may be a
/// single node OR an array of nodes. Each node's `address` is formatted via
/// [`fmt_address`]; multiple addresses join with `"; "`.
fn job_location(node: &serde_json::Value) -> Option<String> {
    let parts: Vec<String> = match node.get("jobLocation") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|n| n.get("address").and_then(fmt_address))
            .collect(),
        Some(single) => single
            .get("address")
            .and_then(fmt_address)
            .into_iter()
            .collect(),
        None => return None,
    };
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Does this node's `@type` denote a `JobPosting`? Tolerates a string or an
/// array of types (schema.org allows multiple types on one node).
fn is_job_posting(node: &serde_json::Value) -> bool {
    match node.get("@type") {
        Some(serde_json::Value::String(s)) => s == "JobPosting",
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("JobPosting")),
        _ => false,
    }
}

/// Extract a [`JsonLdJob`] from a single node IF it is a titled `JobPosting`.
fn job_from_node(node: &serde_json::Value) -> Option<JsonLdJob> {
    if !is_job_posting(node) {
        return None;
    }
    let title = node
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let description = node
        .get("description")
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::html_to_markdown)
        .filter(|s| !s.trim().is_empty());
    Some(JsonLdJob {
        title,
        description,
        location: job_location(node),
    })
}

/// Walk a JSON-LD value (object values, array elements, `@graph` — which is just
/// an object-valued array the recursion descends naturally) for the FIRST titled
/// `JobPosting` node.
// ponytail: depth-12 cap guards cyclic/pathologically-nested JSON-LD; raise it if
// a real page legitimately nests a JobPosting deeper.
fn find_job(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
    if let Some(job) = job_from_node(node) {
        return Some(job);
    }
    if depth >= 12 {
        return None;
    }
    match node {
        serde_json::Value::Array(arr) => arr.iter().find_map(|n| find_job(n, depth + 1)),
        serde_json::Value::Object(map) => map.values().find_map(|n| find_job(n, depth + 1)),
        _ => None,
    }
}

/// Best-effort pull of a JSON-LD `JobPosting` node from anywhere in any
/// `application/ld+json` block (top-level, `@graph`, or arbitrarily nested) —
/// title/description/jobLocation. Returns `None` when no `JobPosting` node
/// carries a usable title.
fn json_ld_job_posting(html: &str) -> Option<JsonLdJob> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for node in doc.select(&sel) {
        let raw = node.text().collect::<String>();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(job) = find_job(&json, 0) {
            return Some(job);
        }
    }
    None
}

/// `__NEXT_DATA__` fallback: Next.js ships the page's props as JSON in a
/// `script#__NEXT_DATA__` blob. Find a `JobPosting` node OR, failing that, a
/// job-shaped node (a non-empty string `title` plus at least one of
/// `description` / `hiringOrganization` / `jobLocation`) and pull the same
/// fields the JSON-LD path does.
// ponytail: Next.js prop-shape sniffing; upgrade path = a per-board JSON path if a
// real page needs one. Same depth-12 cap as `find_job`.
fn next_data_job(html: &str) -> Option<JsonLdJob> {
    /// Pull a `JsonLdJob` out of a job-shaped node (already known to have a title).
    fn from_jobish(node: &serde_json::Value) -> JsonLdJob {
        let title = node
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let description = node
            .get("description")
            .and_then(|s| s.as_str())
            .map(crate::scraping::http::html_to_markdown)
            .filter(|s| !s.trim().is_empty());
        JsonLdJob {
            title,
            description,
            location: job_location(node),
        }
    }
    fn is_jobish(node: &serde_json::Value) -> bool {
        let has_title = node
            .get("title")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        has_title
            && (node.get("description").is_some()
                || node.get("hiringOrganization").is_some()
                || node.get("jobLocation").is_some())
    }
    fn find(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
        // Prefer a real JobPosting; else accept a job-shaped node.
        if let Some(job) = job_from_node(node) {
            return Some(job);
        }
        if is_jobish(node) {
            return Some(from_jobish(node));
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| find(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| find(n, depth + 1)),
            _ => None,
        }
    }

    let doc = Html::parse_document(html);
    let sel = Selector::parse("script#__NEXT_DATA__").ok()?;
    let raw = doc.select(&sel).next()?.text().collect::<String>();
    let json = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    find(&json, 0)
}

/// Real-readability last-resort description: run `dom_smoothie` (a faithful
/// Rust port of Mozilla's Readability.js) over the whole document, so nav/
/// footer/boilerplate get scored out instead of surviving a naive
/// largest-block guess. `is_probably_readable()` must run before `parse()`
/// (it inspects the un-mutated document; `parse()` mutates it) and gates
/// documents too thin to trust — e.g. a nav-only shell with no real article.
/// Both the pre-check and `parse()` are best-effort: any `Err` (bad URL,
/// `max_elements_to_parse` exceeded, no candidate found) falls through to
/// `None` — the caller then tries `main_content_text` — rather than
/// propagating, since this is an enrichment, not a hard requirement.
// TODO(perf): this runs synchronous CPU parsing on the async executor with no
// `spawn_blocking` (same as every other rung of `parse_from_html`'s generic-HTML
// fallback — all use `Html::parse_document`). Deferred: wrap the whole generic
// fallback in `spawn_blocking` at its async call sites (including the
// extension-bridge Scan path) — a broader refactor, out of scope here.
fn readability_content_text(url: &str, html: &str) -> Option<String> {
    // Host only — never log the raw dom_smoothie error, which can embed the
    // scraped `url` (see the `host`-only convention at parse_from_html's own
    // `reqwest::Url::parse` call and the `linkedin` job-id log above).
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let cfg = dom_smoothie::Config {
        // `text_content` becomes ready-to-use markdown straight off the
        // cleaned readability DOM — skips a second html_to_markdown pass over
        // `article.content` (which would re-parse already-cleaned HTML).
        text_mode: dom_smoothie::TextMode::Markdown,
        // Defense-in-depth against a hostile/huge page: default `0` means
        // UNLIMITED, letting an attacker-sized document drive an unbounded
        // multi-pass scoring parse (CPU/peak-memory amplification). 4000 is
        // comfortably above any real job posting page (dom_smoothie's own
        // test suite parses a full Wikipedia article — far denser than a job
        // page — under 10,000 with no false-positive `TooManyElements`)
        // while still bounding abuse. Tripping the cap returns
        // `Err(TooManyElements)`, handled by the `Err ⇒ None` arm below —
        // clean fall-through to `main_content_text`.
        max_elements_to_parse: 4000,
        ..Default::default()
    };
    let mut readability = match dom_smoothie::Readability::new(html, Some(url), Some(cfg)) {
        Ok(r) => r,
        Err(_) => {
            log::debug!(
                "[scraping::scrape_url] dom_smoothie::Readability::new failed for host {host}"
            );
            return None;
        }
    };
    if !readability.is_probably_readable() {
        return None;
    }
    match readability.parse() {
        Ok(article) => {
            let text = article.text_content.trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        Err(_) => {
            log::debug!("[scraping::scrape_url] dom_smoothie parse failed for host {host}");
            None
        }
    }
}

/// Largest main-content text block as a FINAL last-resort description (below
/// `readability_content_text`): pick the longest rendered text among `main` /
/// `[role="main"]` / `article`. Kept as the floor for when readability's own
/// pre-check or `parse()` comes back empty — a naive guess beats nothing.
fn main_content_text(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"main, [role="main"], article"#).ok()?;
    doc.select(&sel)
        .map(|el| crate::scraping::http::html_to_markdown(&el.inner_html()))
        .filter(|t| !t.trim().is_empty())
        .max_by_key(|t| t.len())
}

fn parse_generic_html(html: &str) -> (String, Option<String>) {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title, h1").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let meta_sel =
        Selector::parse("meta[name=\"description\"], meta[property=\"og:description\"]").unwrap();
    let description = doc
        .select(&meta_sel)
        .next()
        .and_then(|e| e.value().attr("content").map(str::to_string));
    (title, description)
}

/// Best-effort real employer name for the generic fallback. Tries JSON-LD
/// (`JobPosting.hiringOrganization.name`, incl. an `@graph` array), then
/// `og:site_name`. Returns `None` when neither is present so the caller can
/// fall back to the host.
fn parse_generic_company(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);

    if let Ok(sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for node in doc.select(&sel) {
            let raw = node.text().collect::<String>();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(name) = json_ld_company(&json) {
                    let name = name.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse(r#"meta[property="og:site_name"]"#) {
        if let Some(name) = doc
            .select(&sel)
            .next()
            .and_then(|e| e.value().attr("content"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(name.to_string());
        }
    }

    None
}

/// Pull `hiringOrganization.name` from a JSON-LD value at any depth, tolerating a
/// single object, a string org, an `@graph` array, and arbitrary nesting.
// ponytail: same depth-12 cap as `find_job` — cyclic/pathological-nesting guard.
fn json_ld_company(json: &serde_json::Value) -> Option<String> {
    fn org_name(node: &serde_json::Value, depth: u8) -> Option<String> {
        match node.get("hiringOrganization") {
            Some(serde_json::Value::String(s)) => return Some(s.clone()),
            Some(org @ serde_json::Value::Object(_)) => {
                if let Some(name) = org.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
            _ => {}
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| org_name(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| org_name(n, depth + 1)),
            _ => None,
        }
    }
    org_name(json, 0)
}

// ── LinkedIn ────────────────────────────────────────────────────────────────
//
// URL: https://www.linkedin.com/jobs/view/<id>
// Requires authed client from board_login::build_authed_client("linkedin").

async fn try_linkedin(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    // Exact/suffix match only. The authed (cookie-bearing) LinkedIn client must
    // *only* ever talk to real `*.linkedin.com` — a substring gate would let a
    // look-alike host (`linkedin.com.attacker.tld`) pass and exfiltrate the
    // user's session cookies cross-host (SSRF + cookie exfil).
    if host != "linkedin.com" && host != "www.linkedin.com" && !host.ends_with(".linkedin.com") {
        return Ok(None);
    }
    let path = u.path();
    if !path.contains("/jobs/view/") {
        return Ok(None);
    }
    let job_id = path.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    if job_id.is_empty() {
        return Ok(None);
    }

    let data_dir = crate::platform::config::data_dir();
    let client = crate::scraping::board_login::build_authed_client(&data_dir, "linkedin")?;

    let res = client.get(url).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let html = res.text().await?;

    let doc = Html::parse_document(&html);
    let title_sel = Selector::parse("h1, .top-card-layout__title").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let company_sel = Selector::parse(".topcard__org-name-link, .top-card-layout__card a").unwrap();
    let company = doc
        .select(&company_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "LinkedIn".to_string());

    let location_sel =
        Selector::parse(".topcard__flavor--bullet, .top-card-layout__second-subline").unwrap();
    let location = doc
        .select(&location_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string());

    let desc_sel = Selector::parse(".show-more-less-html__markup, .description__text").unwrap();
    let description = doc
        .select(&desc_sel)
        .next()
        .map(|e| clean_description(&crate::scraping::http::html_to_text(&e.inner_html())));

    // Selectors miss when LinkedIn ships an auth-gated/redesigned shell. Fall back
    // to the shared JSON-LD / __NEXT_DATA__ / main-content parse and fill ONLY the
    // fields the selectors left empty (keep good selector values, keep source).
    let (title, description, location, company) =
        if title.is_empty() || description.as_deref().unwrap_or("").trim().is_empty() {
            let fb = parse_from_html(url, &html);
            let title = if title.is_empty() {
                fb.as_ref().map(|f| f.title.clone()).unwrap_or_default()
            } else {
                title
            };
            let description = description
                .filter(|d| !d.trim().is_empty())
                .or_else(|| fb.as_ref().and_then(|f| f.description.clone()));
            let location = location.or_else(|| fb.as_ref().and_then(|f| f.location.clone()));
            let company = if company == "LinkedIn" {
                fb.as_ref()
                    .map(|f| f.company.clone())
                    .filter(|c| !c.is_empty())
                    .unwrap_or(company)
            } else {
                company
            };
            (title, description, location, company)
        } else {
            (title, description, location, company)
        };

    log::info!(
        "[scrape_url] linkedin {} description: {} chars",
        job_id,
        description.as_ref().map(|d| d.len()).unwrap_or(0)
    );

    Ok(Some(JobPosting {
        id: format!("linkedin:{}", job_id),
        external_id: Some(job_id.to_string()),
        title,
        company,
        location,
        url: url.to_string(),
        source: "linkedin".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── Workday ─────────────────────────────────────────────────────────────────
//
// URL: https://<tenant>.<host>.myworkdayjobs.com/<site>/job/<...>/<reqId>
// API: /wday/cxs/<tenant>/<site>/job/<reqId>

async fn try_workday(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host_str = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    // Exact/suffix match only — a substring gate (`contains`) would accept a
    // look-alike host (`myworkdayjobs.com.attacker.tld`) which matters now that
    // re-dispatch runs these handlers on attacker-influenced redirect targets.
    if host_str != "myworkdayjobs.com" && !host_str.ends_with(".myworkdayjobs.com") {
        return Ok(None);
    }

    let re = regex::Regex::new(r"^([^.]+)\.(wd\d+)\.myworkdayjobs\.com$").unwrap();
    let caps = match re.captures(host_str) {
        Some(c) => c,
        None => return Ok(None),
    };
    let tenant = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let host = caps.get(2).map(|m| m.as_str()).unwrap_or("wd1");

    let path = u.path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return Ok(None);
    }
    let site = segments[0];
    let req_id = segments.last().unwrap_or(&"");
    if req_id.is_empty() {
        return Ok(None);
    }

    let api = format!(
        "https://{}.{}.myworkdayjobs.com/wday/cxs/{}/{}/job/{}",
        tenant, host, tenant, site, req_id
    );

    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;

    let title = v
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("locationsText")
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("jobPostingInfo")
        .and_then(|info| info.get("jobDescription"))
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::html_to_markdown);

    Ok(Some(JobPosting {
        id: format!("workday:{}", req_id),
        external_id: Some(req_id.to_string()),
        title,
        company: tenant.to_string(),
        location,
        url: url.to_string(),
        source: "workday".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── SmartRecruiters ─────────────────────────────────────────────────────────
//
// URL: https://jobs.smartrecruiters.com/<company>/<id>
// API: https://api.smartrecruiters.com/v1/companies/<company>/postings/<id>

async fn try_smartrecruiters(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    // Exact/suffix match only — same rationale as the Workday gate above:
    // re-dispatch runs this handler on redirect-resolved, attacker-influenceable
    // URLs, so a substring gate widens the surface.
    if host != "smartrecruiters.com" && !host.ends_with(".smartrecruiters.com") {
        return Ok(None);
    }
    let segments: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    if segments.len() < 2 {
        return Ok(None);
    }
    let company = segments[0];
    let job_id = segments[1];

    let api = format!(
        "https://api.smartrecruiters.com/v1/companies/{}/postings/{}",
        urlencoding::encode(company),
        urlencoding::encode(job_id)
    );

    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;

    let title = v
        .get("name")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v.get("location").and_then(|l| {
        let city = l.get("city").and_then(|s| s.as_str());
        let country = l.get("country").and_then(|s| s.as_str());
        match (city, country) {
            (Some(c), Some(co)) => Some(format!("{}, {}", c, co)),
            (Some(c), None) => Some(c.to_string()),
            (None, Some(co)) => Some(co.to_string()),
            _ => None,
        }
    });

    let description = v
        .get("jobAd")
        .and_then(|ja| ja.get("sections"))
        .and_then(|s| s.as_object())
        .map(|sections| {
            sections
                .values()
                .filter_map(|sec| sec.get("text").and_then(|t| t.as_str()))
                .map(crate::scraping::http::html_to_markdown)
                .collect::<Vec<_>>()
                .join("\n\n")
        });

    Ok(Some(JobPosting {
        id: format!("smartrecruiters:{}", job_id),
        external_id: Some(job_id.to_string()),
        title,
        company: company.to_string(),
        location,
        url: url.to_string(),
        source: "smartrecruiters".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── Personio ────────────────────────────────────────────────────────────────
//
// URL: https://<company>.jobs.personio.{de,com}/?id=<id>
// Match against the XML feed item.

/// Extract the company slug from a Personio job URL.
///
/// Valid hosts are `<company>.jobs.personio.{de,com}` — the first host label
/// (before the first `.`) is the company slug, returned lowercased.
/// Returns `None` for non-Personio hosts, bare `jobs.personio.*` roots (no
/// company subdomain), or malformed/unparseable URLs.
pub(crate) fn personio_company_from_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    // Exact/suffix match only — a substring gate would let a look-alike host
    // (`jobs.personio.attacker.tld`) pass.
    if host != "jobs.personio.de"
        && host != "jobs.personio.com"
        && !host.ends_with(".jobs.personio.de")
        && !host.ends_with(".jobs.personio.com")
    {
        return None;
    }
    // The bare roots (`jobs.personio.de` / `jobs.personio.com`) have no
    // company subdomain — the first label would be "jobs", which is wrong.
    if host == "jobs.personio.de" || host == "jobs.personio.com" {
        return None;
    }
    let company = host.split('.').next()?;
    if company.is_empty() {
        return None;
    }
    Some(company.to_ascii_lowercase())
}

async fn try_personio(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };

    let company = match personio_company_from_url(url) {
        Some(c) => c,
        None => return Ok(None),
    };

    let id = u
        .query_pairs()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.into_owned());
    let id = match id {
        Some(i) if !i.is_empty() => i,
        _ => return Ok(None),
    };

    // Fetch the XML feed and find the matching position.
    let feed_url = format!("https://{}", host);
    // Route through the IP-validated, IP-pinned, redirect-disabled guarded
    // client: even with the tightened host gate, treat the feed fetch as an
    // attacker-influenced egress and close the DNS-rebinding TOCTOU.
    let res = match crate::net::http::get_guarded(&feed_url).await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !res.status().is_success() {
        return Ok(None);
    }
    let xml = res.text().await?;

    // Shared feed parser (regex set + capture loop) lives in the Personio board.
    // Here we pick the single position whose id matches the URL query and map it
    // onto this resolver's JobPosting shape (original url, personio:{company}:{id}).
    // Use make_job_id so both the board-scrape path and this URL-resolve path
    // produce byte-identical ids for the same posting — deduplication depends on it.
    let position = crate::scraping::boards::personio::parse_xml_feed(&xml)
        .into_iter()
        .find(|p| p.id == id);
    if let Some(pos) = position {
        return Ok(Some(JobPosting {
            id: crate::scraping::boards::personio::make_job_id(&company, &id),
            external_id: Some(id.clone()),
            title: pos.title,
            company: company.clone(),
            location: if pos.office.is_empty() {
                None
            } else {
                Some(pos.office)
            },
            url: url.to_string(),
            source: "personio".to_string(),
            description: if pos.description.is_empty() {
                None
            } else {
                Some(pos.description)
            },
            requirements: None,
            posted_at: None,
            captured_at: chrono::Utc::now().timestamp_millis(),
            extra: HashMap::new(),
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod test;
