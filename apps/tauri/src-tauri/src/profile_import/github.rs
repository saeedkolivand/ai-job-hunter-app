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

/// Fetch a user's public repos, drop forks, sort by stars (desc), cap to 30.
///
/// `input` may be a bare username or a `github.com/<user>` URL. We extract +
/// validate the username, then build the api.github.com URL ourselves.
///
/// The GET goes through the hardened [`fetch_text`] helper, which gives the 8 MB
/// streaming body cap, the per-host rate limiter, and cancellation for free; we
/// add an explicit [`REQUEST_TIMEOUT`] since the shared client has no global one.
/// We keep the status code so the 404 / 403,429 mapping survives (which
/// `fetch_json` would collapse into a single `None`).
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
fn api_url(username: &str) -> String {
    format!("https://api.github.com/users/{username}/repos?per_page=100&sort=updated&type=owner")
}

/// Drop forks, sort by stars descending (name as a stable tiebreaker), cap to
/// [`MAX_REPOS`], and map to the output struct.
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
mod tests {
    use super::*;

    fn repo(name: &str, stars: u64, fork: bool) -> RawRepo {
        RawRepo {
            name: name.to_string(),
            description: None,
            html_url: format!("https://github.com/u/{name}"),
            language: None,
            topics: vec![],
            stargazers_count: stars,
            pushed_at: None,
            fork,
        }
    }

    // ── username extraction ───────────────────────────────────────────────────

    #[test]
    fn parse_username_from_bare_name() {
        assert_eq!(parse_username("torvalds").unwrap(), "torvalds");
    }

    #[test]
    fn parse_username_trims_whitespace() {
        assert_eq!(parse_username("  octocat  ").unwrap(), "octocat");
    }

    #[test]
    fn parse_username_from_https_url() {
        assert_eq!(
            parse_username("https://github.com/torvalds").unwrap(),
            "torvalds"
        );
    }

    #[test]
    fn parse_username_from_url_with_extra_path() {
        // Only the first path segment (the user) is taken, not the repo.
        assert_eq!(
            parse_username("https://github.com/torvalds/linux").unwrap(),
            "torvalds"
        );
    }

    #[test]
    fn parse_username_from_scheme_less_url() {
        assert_eq!(parse_username("github.com/octocat").unwrap(), "octocat");
    }

    #[test]
    fn parse_username_strips_trailing_query() {
        assert_eq!(
            parse_username("https://github.com/octocat?tab=repositories").unwrap(),
            "octocat"
        );
    }

    // ── rejection of invalid / hostile input (SSRF guard) ─────────────────────

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            parse_username("   "),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_path_traversal() {
        // `../foo` must never become a username — first segment is `..`.
        assert!(matches!(
            parse_username("../foo"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_foreign_host_url() {
        // A non-github host is rejected outright — its path is never even read as a
        // candidate username, and the host is never forwarded to the HTTP client.
        assert!(matches!(
            parse_username("https://evil.com"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            parse_username("https://evil.com/torvalds"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn url_preserves_username_case() {
        assert_eq!(
            parse_username("https://github.com/TorValds").unwrap(),
            "TorValds"
        );
    }

    #[test]
    fn parse_username_from_www_and_mixed_case_scheme() {
        assert_eq!(
            parse_username("HTTPS://www.github.com/octocat").unwrap(),
            "octocat"
        );
    }

    #[test]
    fn rejects_metadata_url() {
        // `http://169.254.169.254/` — cloud metadata. No path segment → rejected,
        // and even with one it would have to pass the username regex.
        assert!(matches!(
            parse_username("http://169.254.169.254/"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_leading_hyphen() {
        assert!(matches!(
            parse_username("-bad"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_trailing_hyphen() {
        assert!(matches!(
            parse_username("bad-"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_double_hyphen() {
        assert!(matches!(
            parse_username("a--b"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_over_39_chars() {
        let long = "a".repeat(40);
        assert!(matches!(
            parse_username(&long),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_disallowed_chars() {
        assert!(matches!(
            parse_username("foo_bar"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            parse_username("foo.bar"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn accepts_max_length_and_internal_hyphen() {
        assert_eq!(parse_username("a-b").unwrap(), "a-b");
        let max = "a".repeat(39);
        assert_eq!(parse_username(&max).unwrap(), max);
    }

    // ── URL construction ──────────────────────────────────────────────────────

    #[test]
    fn api_url_is_constructed_from_username() {
        assert_eq!(
            api_url("octocat"),
            "https://api.github.com/users/octocat/repos?per_page=100&sort=updated&type=owner"
        );
    }

    // ── filter + sort ─────────────────────────────────────────────────────────

    #[test]
    fn forks_are_dropped() {
        let raw = vec![repo("real", 5, false), repo("forked", 100, true)];
        let out = filter_and_rank(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "real");
    }

    #[test]
    fn sorts_by_stars_descending() {
        let raw = vec![
            repo("low", 1, false),
            repo("high", 99, false),
            repo("mid", 50, false),
        ];
        let out = filter_and_rank(raw);
        let names: Vec<&str> = out.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["high", "mid", "low"]);
    }

    #[test]
    fn equal_stars_break_ties_by_name() {
        let raw = vec![repo("zeta", 10, false), repo("alpha", 10, false)];
        let out = filter_and_rank(raw);
        assert_eq!(out[0].name, "alpha");
        assert_eq!(out[1].name, "zeta");
    }

    #[test]
    fn caps_at_30() {
        let raw: Vec<RawRepo> = (0..50)
            .map(|i| repo(&format!("repo{i:02}"), i, false))
            .collect();
        let out = filter_and_rank(raw);
        assert_eq!(out.len(), MAX_REPOS);
        // Highest-starred must survive the cap.
        assert_eq!(out[0].stars, 49);
    }

    #[test]
    fn maps_stargazers_count_to_stars() {
        let out = filter_and_rank(vec![repo("r", 7, false)]);
        assert_eq!(out[0].stars, 7);
    }

    // ── HTTP status mapping (item 1: pure map_status helper) ─────────────────

    #[test]
    fn map_status_404_is_validation() {
        assert!(matches!(map_status(404), Some(AppError::Validation(_))));
        if let Some(AppError::Validation(msg)) = map_status(404) {
            assert!(
                msg.to_lowercase().contains("not found"),
                "expected 'not found' in {msg:?}"
            );
        }
    }

    #[test]
    fn map_status_403_is_rate_limited() {
        assert!(matches!(map_status(403), Some(AppError::RateLimited(_))));
    }

    #[test]
    fn map_status_429_is_rate_limited() {
        assert!(matches!(map_status(429), Some(AppError::RateLimited(_))));
    }

    #[test]
    fn map_status_500_is_network() {
        assert!(matches!(map_status(500), Some(AppError::Network(_))));
    }

    #[test]
    fn map_status_503_is_network() {
        assert!(matches!(map_status(503), Some(AppError::Network(_))));
    }

    #[test]
    fn map_status_200_is_none() {
        assert!(map_status(200).is_none());
    }

    #[test]
    fn map_status_201_is_none() {
        assert!(map_status(201).is_none());
    }

    #[test]
    fn map_status_299_is_none() {
        assert!(map_status(299).is_none());
    }

    // ── parse_username SSRF edge cases (item 2) ───────────────────────────────

    #[test]
    fn rejects_bare_host_no_path_segment() {
        // "https://github.com" — host only, no slash-after-host path segment.
        assert!(matches!(
            parse_username("https://github.com"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_trailing_slash_empty_segment() {
        // "https://github.com/" — trailing slash yields an empty first segment.
        assert!(matches!(
            parse_username("https://github.com/"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rejects_url_segment_invalid_github_username() {
        // "https://github.com/-bad" — valid URL segment but invalid GitHub username
        // (leading hyphen). Two-stage rejection: URL parse succeeds, username validate fails.
        assert!(matches!(
            parse_username("https://github.com/-bad"),
            Err(AppError::Validation(_))
        ));
    }

    // ── serde: Some case (item 3) ─────────────────────────────────────────────

    #[test]
    fn output_some_pushed_at_is_present_and_camel_cased() {
        let repo = GitHubRepo {
            name: "my-project".to_string(),
            description: Some("A great project".to_string()),
            html_url: "https://github.com/u/my-project".to_string(),
            language: None,
            topics: vec![],
            stars: 1,
            pushed_at: Some("2026-01-10T12:00:00Z".to_string()),
        };
        let v = serde_json::to_value(&repo).unwrap();
        let obj = v.as_object().unwrap();
        // `Some` description must appear (not omitted).
        assert_eq!(obj["description"], "A great project");
        // `Some` pushedAt serialized under camelCase key — a future rename to
        // `pushed_at` (snake) or `PushedAt` is caught by this assertion.
        assert_eq!(obj["pushedAt"], "2026-01-10T12:00:00Z");
        // No snake_case leakage.
        assert!(
            !obj.contains_key("pushed_at"),
            "snake_case key must not appear"
        );
    }

    // ── output serialization ──────────────────────────────────────────────────

    #[test]
    fn output_omits_none_fields_and_camel_cases() {
        // `None` Options are omitted (skip_serializing_if) so the TS contract is
        // `description?: string`, not `string | null`; `htmlUrl`/`pushedAt` are
        // camelCase; `stargazers_count` is exposed as `stars`.
        let repo = GitHubRepo {
            name: "linux".to_string(),
            description: None,
            html_url: "https://github.com/torvalds/linux".to_string(),
            language: Some("C".to_string()),
            topics: vec!["kernel".to_string()],
            stars: 42,
            pushed_at: None,
        };
        let v = serde_json::to_value(&repo).unwrap();
        let obj = v.as_object().unwrap();

        assert!(!obj.contains_key("description"), "None must be omitted");
        assert!(!obj.contains_key("pushedAt"), "None must be omitted");
        assert!(!obj.contains_key("pushed_at"), "no snake_case key");
        assert_eq!(obj["htmlUrl"], "https://github.com/torvalds/linux");
        assert_eq!(obj["language"], "C");
        assert_eq!(obj["stars"], 42);
        assert!(!obj.contains_key("stargazers_count"), "renamed to stars");
    }
}
