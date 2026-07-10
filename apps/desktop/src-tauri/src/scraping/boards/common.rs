//! Shared helpers for company-scoped ATS board scrapers (Ashby, BambooHR,
//! Breezy, Greenhouse, Pinpoint, Rippling, SmartRecruiters). Extracted from
//! 7 byte-identical copies of `normalize_companies` and 2 byte-identical
//! copies of `is_https_url` — see `.claude/scratch/scraping-followups.md`.
//! Per-board `is_valid_<board>_slug` validators are mostly NOT here: most
//! genuinely differ per board (DNS-label vs URL-path-segment rules, subdomain
//! vs path-segment interpolation — e.g. Personio, Recruitee, Rippling,
//! Workable) by design. The exception is [`is_valid_dns_label_slug`]: BambooHR,
//! Breezy, and Pinpoint all validate a subdomain-interpolated slug with the
//! exact same DNS-label character-set rule, so that one shape is extracted
//! here and shared instead of kept as 3 more byte-identical copies.

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

/// Validate that a company slug is a single valid DNS hostname label
/// (alphanumeric + hyphen, max 63 chars, no leading/trailing hyphen). Used by
/// boards (BambooHR, Breezy, Pinpoint) that interpolate the slug as a
/// SUBDOMAIN — a slug with dots, slashes, or colons could change the URL
/// authority and redirect the fetch away from the target host (SSRF).
pub(crate) fn is_valid_dns_label_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

/// Client-side query/location filter for boards with no server-side keyword
/// search (The Muse, Comeet). Case-insensitive substring match on
/// `title + company` for `query`; case-insensitive substring match on
/// `location` for `location`; an empty filter passes everything; both
/// clauses AND-combine. Originally The Muse-local; extracted here once
/// Comeet needed the identical filter instead of a second copy.
pub(crate) fn matches_filters(
    posting: &crate::scraping::types::JobPosting,
    query: &str,
    location: &str,
) -> bool {
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        let haystack = format!("{} {}", posting.title, posting.company).to_lowercase();
        if !haystack.contains(&q) {
            return false;
        }
    }

    let loc_filter = location.trim().to_lowercase();
    if !loc_filter.is_empty() {
        let loc = posting.location.as_deref().unwrap_or("").to_lowercase();
        if !loc.contains(&loc_filter) {
            return false;
        }
    }

    true
}

/// Decide whether an ATS multi-company board's fetch loop should fail the
/// whole board. Returns `Some(message)` — ready to wrap in
/// `anyhow::anyhow!` — only when every attempted per-company fetch failed
/// (`successful_fetches == 0` AND at least one real fetch error was
/// recorded). Returns `None` when at least one company succeeded (partial
/// success is kept) OR when no company ever reached a fetch (e.g. every slug
/// was rejected by a pre-fetch validation guard, or the input list was
/// empty) — that is a "nothing to fetch" outcome, not a board failure.
///
/// Pure — no I/O, no board host — so it is directly unit-testable without a
/// mock server. Shared by every ATS board that tracks
/// `successful_fetches`/`first_fetch_error` (Ashby, BambooHR, Breezy,
/// Greenhouse, Lever, Pinpoint, Recruitee, Rippling, SmartRecruiters,
/// Workable).
pub(crate) fn ats_all_fetches_failed(
    board_id: &str,
    successful_fetches: usize,
    first_fetch_error: &Option<String>,
) -> Option<String> {
    if successful_fetches > 0 {
        return None;
    }
    first_fetch_error
        .as_ref()
        .map(|error| format!("all {board_id} company fetches failed: {error}"))
}

/// Decide the pagination-failure policy for a page fetch: a page reached with
/// nothing collected yet (typically page 0) propagates the fetch failure as a
/// board error; a later page that fails after some items were already
/// streamed instead stops pagination and keeps the partial result.
///
/// Pure — takes only the already-known collected count, no I/O — so it is
/// directly unit-testable without a mock server. Shared by every paginated
/// board that fails closed on an empty-so-far collection (The Muse,
/// Arbeitnow, Arbeitsagentur).
pub(crate) fn should_propagate_page_error(collected_so_far: usize) -> bool {
    collected_so_far == 0
}

#[cfg(test)]
mod test;
