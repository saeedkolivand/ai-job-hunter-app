//! Shared `JobPosting` fixtures for the trust-assessment test topics.

use std::collections::HashMap;

use super::super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal `JobPosting` fixture — mirrors the shape a board scraper returns.
/// Carries a non-empty `description` by default (real boards do) so callers
/// exercising unrelated behavior aren't also tripping the
/// `DescriptionUnavailable` flag; [`posting_without_description`] below is
/// the dedicated fixture for that.
pub(super) fn posting(url: &str, company: &str) -> JobPosting {
    JobPosting {
        id: "job-1".to_string(),
        external_id: None,
        title: "Backend Engineer".to_string(),
        company: company.to_string(),
        location: None,
        url: url.to_string(),
        source: "manual".to_string(),
        description: Some("Own the backend roadmap for a growing team.".to_string()),
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: HashMap::new(),
    }
}

/// Same as [`posting`] but with no description — the LinkedIn free/guest
/// board's real shape (`description: Some(String::new())`) as well as the
/// legacy `None` case.
pub(super) fn posting_without_description(url: &str, company: &str) -> JobPosting {
    let mut p = posting(url, company);
    p.description = None;
    p
}
