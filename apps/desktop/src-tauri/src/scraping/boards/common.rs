//! Shared helpers for company-scoped ATS board scrapers (Ashby, BambooHR,
//! Breezy, Greenhouse, Pinpoint, Rippling, SmartRecruiters). Extracted from
//! 7 byte-identical copies of `normalize_companies` and 2 byte-identical
//! copies of `is_https_url` — see `.claude/scratch/scraping-followups.md`.
//! Per-board `is_valid_<board>_slug` validators are intentionally NOT here —
//! they differ per board (DNS-label vs URL-path-segment rules) by design.

/// Trim, drop blanks, dedupe (first-seen order), and cap to `max`.
/// Extracted so the normalisation logic can be unit-tested without network.
pub(crate) fn normalize_companies(input: &[String], max: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    input
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .take(max)
        .collect()
}

/// Require a well-formed `https:` URL with a host and no embedded userinfo.
/// Used by boards (Pinpoint, Breezy) whose response `url` field is
/// display-only (used as a dedup key, not fetched by us), so this is a cheap
/// sanity parse, not a host allowlist — but a
/// `https://user:pass@evil.example/…` URL is still a mild phishing vector on
/// a link the user opens, so userinfo is rejected outright.
pub(crate) fn is_https_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .unwrap_or(false)
}
