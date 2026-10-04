//! GitHub public-repo fetch for the resume-builder "Import from GitHub" flow.
//!
//! Returns the user's public, non-fork repos (top 30 by stars) so the renderer
//! can let the candidate multi-select projects to add. This is *not* a profile
//! import — it returns repos, not a [`super::ProfileData`], so it deliberately
//! stays out of `detect_platform` / `import_from_url`.
//!
//! **SSRF posture:** the only network egress is to a URL we construct ourselves
//! from a validated username (`^[A-Za-z0-9-]{1,39}$`, GitHub's own rule). A
//! user-supplied `github.com/<user>` URL is parsed only to *extract* the
//! username — it is never forwarded to the HTTP client, so a hostile
//! `https://evil.com/path` or `http://169.254.169.254/` can't reach the wire.

use serde::{Deserialize, Serialize};

use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::scraping::http::{fetch_text, FetchOptions};

/// Output struct sent to the renderer — camelCase to match the TS contract.
/// `None` fields are omitted (mirrors `contact_profile`) so the TS side sees
/// `description?: string` rather than `string | null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub html_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub topics: Vec<String>,
    pub stars: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushed_at: Option<String>,
}

/// Raw shape decoded from the GitHub REST API (snake_case as the API returns it).
/// Kept separate from [`GitHubRepo`] so the wire→output rename is explicit.
#[derive(Debug, Clone, Deserialize)]
struct RawRepo {
    name: String,
    #[serde(default)]
    description: Option<String>,
    html_url: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    fork: bool,
}

impl From<RawRepo> for GitHubRepo {
    fn from(r: RawRepo) -> Self {
        GitHubRepo {
            name: r.name,
            description: r.description,
            html_url: r.html_url,
            language: r.language,
            topics: r.topics,
            stars: r.stargazers_count,
            pushed_at: r.pushed_at,
        }
    }
}

/// Top-N cap returned to the renderer.
const MAX_REPOS: usize = 30;

/// Per-request wall-clock ceiling for the GitHub egress.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Map a GitHub HTTP status code to the appropriate [`AppError`], or `None` for a
/// successful 2xx response. Pure helper extracted from `fetch_repos` so it can be
/// unit-tested without a network call.
///
/// - 404 → `Validation("GitHub user not found")`
/// - 403 / 429 → `RateLimited` (unauthenticated cap is 60 req/hr; 429 on newer secondary-limit)
/// - any other non-2xx → `Network("Failed to reach GitHub")` (fixed message, no status leak)
/// - 2xx → `None`
fn map_status(code: u16) -> Option<AppError> {
    match code {
        200..=299 => None,
        404 => Some(AppError::Validation("GitHub user not found".to_string())),
        403 | 429 => Some(AppError::RateLimited(
            "GitHub rate limit reached, try again later".to_string(),
        )),
        _ => Some(AppError::Network("Failed to reach GitHub".to_string())),
    }
}

/// Fetch a user's public repos and return the top [`MAX_REPOS`] by stars.
///
/// **Ranking window (v1, no pagination):** we fetch a SINGLE page of the 100
/// most-recently-updated owner repos (`per_page=100&sort=updated&type=owner`),
/// then [`filter_and_rank`] drops forks and re-sorts that slice by stars. So the
/// result is "top 30 by stars **among the 100 most-recently-updated** repos",
/// NOT a true global top-30-by-stars — a long-dormant high-star repo outside the
/// recency window won't appear. This is deliberate v1 scope; paginating the full
/// repo set would be the follow-up if global ranking is ever needed.
///
/// `input` may be a bare username or a `github.com/<user>` URL. We extract +
/// validate the username, then build the api.github.com URL ourselves.
///
/// The GET goes through the hardened [`fetch_text`] helper, which gives the 8 MB
/// streaming body cap, the per-host rate limiter, and cancellation for free; we
/// add an explicit [`REQUEST_TIMEOUT`] since the shared client has no global one.
/// We use `fetch_text` (not `fetch_json`) specifically to keep `res.status_code`
/// as a plain number we can branch on in [`map_status`] for GitHub's own
/// 404-vs-403/429-vs-other semantics — `fetch_json` now turns every non-2xx into
/// one `Err(AppError::Provider("HTTP <status>"))`, which is the right contract
/// for the scraping boards but loses the structured code this 404/403/429
/// distinction needs.
pub async fn fetch_repos(input: &str) -> AppResult<Vec<GitHubRepo>> {
    let username = parse_username(input)?;
    let url = api_url(&username);

    // No live cancel signal at this layer — a fresh token keeps fetch_text's
    // cancellation plumbing happy without ever firing.
    let signal = tokio_util::sync::CancellationToken::new();
    let res = fetch_text(
        &url,
        FetchOptions {
            // Setting `accept` here suppresses fetch_text's broad HTML accept.
            headers: Some(vec![(
                "accept".to_string(),
                "application/vnd.github+json".to_string(),
            )]),
            timeout: Some(REQUEST_TIMEOUT),
            ..FetchOptions::default()
        },
        signal,
    )
    .await
    // Fixed message — never echo the request URL (which carries the username)
    // back to the renderer via reqwest's error string.
    .map_err(|_| AppError::Network("Failed to reach GitHub".to_string()))?;

    let status = res.status_code;
    if let Some(err) = map_status(status) {
        return Err(err);
    }

    let raw: Vec<RawRepo> =
        serde_json::from_str(&res.text).map_err(|e| AppError::Parse(e.to_string()))?;

    Ok(filter_and_rank(raw))
}

/// Extract + validate the GitHub username from a bare name or a profile URL.
///
/// A bare username never contains `/`, so any slash-bearing input MUST be a
/// `github.com/<user>` URL — we take the first path segment after the
/// `github.com` host and reject everything else (a foreign host, `../foo`
/// traversal, a metadata URL). This is stricter than a generic
/// first-path-segment parse and is the SSRF guard: a non-github URL never even
/// yields a candidate username. The returned name always satisfies GitHub's
/// `^[A-Za-z0-9-]{1,39}$` rule (no leading/trailing hyphen, no `--`).
fn parse_username(input: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "GitHub username is required".to_string(),
        ));
    }

    let candidate = if trimmed.contains('/') {
        // Slash present → must be a github.com URL; anything else is rejected.
        github_url_first_segment(trimmed)
            .ok_or_else(|| AppError::Validation(format!("not a GitHub profile URL: {trimmed:?}")))?
    } else {
        trimmed.to_string()
    };

    validate_username(&candidate)?;
    Ok(candidate)
}

/// First path segment of a `github.com/<user>/…` URL, or `None` if the host is
/// not github.com (or there's no segment after it). The host is compared
/// case-insensitively; the returned segment keeps its original casing.
///
/// `https://github.com/torvalds` → `Some("torvalds")`;
/// `github.com/torvalds/linux?tab=x` → `Some("torvalds")`;
/// `https://evil.com/foo` / `../foo` / `http://169.254.169.254/` → `None`.
fn github_url_first_segment(input: &str) -> Option<String> {
    let trimmed = input.trim();
    // Strip an http(s):// scheme case-insensitively, preserving the rest as-is.
    let no_scheme = trimmed
        .get(..8)
        .filter(|p| p.eq_ignore_ascii_case("https://"))
        .map(|_| &trimmed[8..])
        .or_else(|| {
            trimmed
                .get(..7)
                .filter(|p| p.eq_ignore_ascii_case("http://"))
                .map(|_| &trimmed[7..])
        })
        .unwrap_or(trimmed);

    let mut parts = no_scheme.splitn(2, '/');
    let host = parts.next()?.trim_start_matches("www.");
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let path = parts.next()?;

    // First non-empty path segment, stripped of any trailing query/fragment.
    let seg = path
        .split('/')
        .map(str::trim)
        .find(|s| !s.is_empty())?
        .split(['?', '#'])
        .next()?;
    if seg.is_empty() {
        return None;
    }
    Some(seg.to_string())
}

/// Enforce GitHub's username rule: 1–39 chars of `[A-Za-z0-9-]`, no leading or
/// trailing hyphen, and no consecutive hyphens (`--`).
fn validate_username(name: &str) -> AppResult<()> {
    let invalid = || AppError::Validation(format!("invalid GitHub username: {name:?}"));

    if name.is_empty() || name.len() > 39 {
        return Err(invalid());
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err(invalid());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(invalid());
    }
    Ok(())
}

/// Build the api.github.com URL ourselves from a validated username — the only
/// URL ever handed to the HTTP client.
///
/// Requests a SINGLE page of the 100 most-recently-updated owner repos
/// (`per_page=100&sort=updated&type=owner`). No pagination (v1), so this is the
/// recency window [`filter_and_rank`] then ranks by stars — see [`fetch_repos`].
fn api_url(username: &str) -> String {
    format!("https://api.github.com/users/{username}/repos?per_page=100&sort=updated&type=owner")
}

/// Drop forks, sort by stars descending (name as a stable tiebreaker), cap to
/// [`MAX_REPOS`], and map to the output struct.
///
/// NOTE: this ranks ONLY the slice handed to it — the 100 most-recently-updated
/// repos from [`api_url`] — so the output is "top [`MAX_REPOS`] by stars among
/// the recency window", not a global top-by-stars. See [`fetch_repos`] for the
/// rationale (v1 scope, no pagination).
fn filter_and_rank(raw: Vec<RawRepo>) -> Vec<GitHubRepo> {
    let mut repos: Vec<GitHubRepo> = raw
        .into_iter()
        .filter(|r| !r.fork)
        .map(GitHubRepo::from)
        .collect();
    repos.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
    repos.truncate(MAX_REPOS);
    repos
}

#[cfg(test)]
mod tests;
