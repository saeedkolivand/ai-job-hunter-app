//! `extract_ats_ref` — the single URL-shape authority that maps a job-posting
//! URL to the ATS board id + company slug it belongs to (ADR-030 §a).
//!
//! Feeds passive slug harvesting (`crate::discovered`). Wherever `scrape_url`
//! already encodes an ATS URL shape for single-job resolution (Greenhouse,
//! Lever, Ashby, SmartRecruiters, Personio) the slug rule lives HERE and is
//! shared by both, so there is ONE authority per ATS, never a fork:
//! `scrape_url`'s `parse_greenhouse_url`/`parse_lever_url`/`try_ashby`/
//! `try_smartrecruiters` call these `*_slug` fns for the company, then layer the
//! job id on top; `personio` is reused directly via `personio_company_from_url`.
//!
//! Host matching is case-insensitive (the `url` crate already lowercases the
//! host); slug casing is preserved EXACTLY — Ashby's board tokens are
//! case-sensitive (`Linear`, `Perplexity`).

use crate::scraping::scrape_url::personio_company_from_url;

/// A company reference extracted from a URL: the registry board id, the company
/// slug (casing preserved), and an optional display name IF the URL itself
/// carried one. No supported shape carries a display name today, so this is
/// always `None` from [`extract_ats_ref`] and the caller passes the posting's
/// company — the field exists so a future shape (e.g. `?company=Acme%20Inc`)
/// needs no signature change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtsRef {
    /// Registry board id (matches a `Scraper::id()` in `SCRAPERS`).
    pub ats: String,
    pub slug: String,
    pub display_name: Option<String>,
}

/// A per-ATS pure slug parser: gates the host and returns the company slug
/// (casing preserved) or `None`.
type SlugParser = fn(&str) -> Option<String>;

/// Parse a URL to its `(ats, slug)` company reference, or `None` when it is not a
/// recognised company-scoped ATS careers/posting URL. The ATS hosts are disjoint,
/// so probe order is irrelevant; the first match wins.
pub fn extract_ats_ref(url: &str) -> Option<AtsRef> {
    // `(board_id, slug_parser)`. Each parser gates the host and returns the slug
    // with its original casing (or `None`). `personio_company_from_url` is reused
    // verbatim from `scrape_url` (already the personio authority).
    const PARSERS: &[(&str, SlugParser)] = &[
        ("greenhouse", greenhouse_slug),
        ("lever", lever_slug),
        ("ashby", ashby_slug),
        ("smartrecruiters", smartrecruiters_slug),
        ("personio", personio_company_from_url),
        ("workable", workable_slug),
        ("recruitee", recruitee_slug),
        ("breezy", breezy_slug),
        ("bamboohr", bamboohr_slug),
        ("pinpoint", pinpoint_slug),
        ("rippling", rippling_slug),
    ];
    for (ats, parse) in PARSERS {
        if let Some(slug) = parse(url) {
            return Some(AtsRef {
                ats: (*ats).to_string(),
                slug,
                display_name: None,
            });
        }
    }
    None
}

/// First path segment (index 0), or `None` when the path is empty. Casing is
/// preserved (the path is never lowercased by the `url` crate).
fn first_segment(u: &reqwest::Url) -> Option<String> {
    u.path_segments()
        .and_then(|mut segs| segs.next())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// The single leading DNS label of a host directly under `dot_suffix`
/// (e.g. `acme` from `acme.recruitee.com`, suffix `.recruitee.com`). `None` for
/// the bare suffix host, a multi-level host (`x.y.recruitee.com`), a look-alike
/// (`evil-recruitee.com` never ends with `.recruitee.com`), or a `www.` front.
/// The host is already lowercase (DNS labels are case-insensitive).
fn subdomain_slug(url: &str, dot_suffix: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let label = host.strip_suffix(dot_suffix)?;
    if label.is_empty() || label.contains('.') || label == "www" {
        return None;
    }
    Some(label.to_string())
}

/// Greenhouse: `boards.greenhouse.io` / `job-boards.greenhouse.io` /
/// `boards.eu.greenhouse.io` only (rejects `www.greenhouse.io`, the bare apex,
/// and `greenhouse.io/blog/…` marketing paths). Slug = first path segment
/// (`/{slug}` careers page or `/{slug}/jobs/{id}` posting), or the `for` query
/// param on the `/embed/job_app` widget.
pub(crate) fn greenhouse_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if host != "boards.greenhouse.io"
        && host != "job-boards.greenhouse.io"
        && host != "boards.eu.greenhouse.io"
    {
        return None;
    }
    let seg = first_segment(&u)?;
    if seg == "embed" {
        return u
            .query_pairs()
            .find(|(k, _)| k == "for")
            .map(|(_, v)| v.into_owned())
            .filter(|s| !s.is_empty());
    }
    Some(seg)
}

/// Lever: `jobs.lever.co` (and any `*.lever.co` subdomain), slug = first path
/// segment (`jobs.lever.co/{slug}` or `jobs.lever.co/{slug}/{id}`). The apex
/// `lever.co` and look-alikes (`evillever.co`) are rejected.
pub(crate) fn lever_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if !host.ends_with(".lever.co") {
        return None;
    }
    first_segment(&u)
}

/// Ashby: `jobs.ashbyhq.com` (and any `*.ashbyhq.com`), slug = first path
/// segment. CASING IS SIGNIFICANT and preserved (`Linear`, `Perplexity`).
pub(crate) fn ashby_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if !host.ends_with(".ashbyhq.com") {
        return None;
    }
    first_segment(&u)
}

/// SmartRecruiters: `jobs.smartrecruiters.com` / `careers.smartrecruiters.com`
/// (any `*.smartrecruiters.com`), company identifier = first path segment. Casing
/// preserved.
pub(crate) fn smartrecruiters_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if !host.ends_with(".smartrecruiters.com") {
        return None;
    }
    first_segment(&u)
}

/// Workable: `apply.workable.com/{slug}/…`, account slug = first path segment.
fn workable_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if host != "apply.workable.com" {
        return None;
    }
    first_segment(&u)
}

/// Recruitee: `{slug}.recruitee.com`.
fn recruitee_slug(url: &str) -> Option<String> {
    subdomain_slug(url, ".recruitee.com")
}

/// Breezy HR: `{slug}.breezy.hr`.
fn breezy_slug(url: &str) -> Option<String> {
    subdomain_slug(url, ".breezy.hr")
}

/// BambooHR: `{slug}.bamboohr.com`.
fn bamboohr_slug(url: &str) -> Option<String> {
    subdomain_slug(url, ".bamboohr.com")
}

/// Pinpoint: `{slug}.pinpointhq.com`.
fn pinpoint_slug(url: &str) -> Option<String> {
    subdomain_slug(url, ".pinpointhq.com")
}

/// Rippling: posting URLs are host-locked to `ats.rippling.com`
/// (`ats.rippling.com/{slug}/jobs/{id}` — verified in `boards::rippling`'s
/// `is_valid_rippling_job_url` guard + fixtures), company identifier = the first
/// path segment. Casing preserved — Rippling board slugs are URL path segments,
/// mixed case allowed (see `is_valid_rippling_slug`), NOT DNS labels. The API host
/// `api.rippling.com` (whose first path segment is `platform`, not a slug) and the
/// apex/look-alikes are rejected by the exact-host gate.
fn rippling_slug(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if host != "ats.rippling.com" {
        return None;
    }
    let slug = first_segment(&u)?;
    // Validate against the SAME shape the board enforces (`is_valid_rippling_slug`)
    // so we never persist a slug `boards::rippling` would later refuse — e.g. a
    // path-traversal/query-bearing or over-length first segment.
    crate::scraping::boards::rippling::is_valid_rippling_slug(&slug).then_some(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert a URL extracts to `(ats, slug)` with no display name.
    fn assert_ref(url: &str, ats: &str, slug: &str) {
        assert_eq!(
            extract_ats_ref(url),
            Some(AtsRef {
                ats: ats.to_string(),
                slug: slug.to_string(),
                display_name: None,
            }),
            "URL {url} must extract to ({ats}, {slug})"
        );
    }

    // ── One positive per documented pattern (verified vs SCRAPING_ENDPOINTS.md) ──

    #[test]
    fn greenhouse_all_three_hosts() {
        assert_ref(
            "https://boards.greenhouse.io/stripe",
            "greenhouse",
            "stripe",
        );
        assert_ref(
            "https://job-boards.greenhouse.io/airbnb/jobs/456",
            "greenhouse",
            "airbnb",
        );
        assert_ref(
            "https://boards.eu.greenhouse.io/celonis",
            "greenhouse",
            "celonis",
        );
        // Deep posting URL (what a harvested `absolute_url` looks like).
        assert_ref(
            "https://boards.greenhouse.io/gitlab/jobs/12345",
            "greenhouse",
            "gitlab",
        );
        // Embed widget carries the slug in `?for=`.
        assert_ref(
            "https://boards.greenhouse.io/embed/job_app?for=dropbox&token=99",
            "greenhouse",
            "dropbox",
        );
    }

    #[test]
    fn lever_positive() {
        assert_ref("https://jobs.lever.co/spotify", "lever", "spotify");
        assert_ref(
            "https://jobs.lever.co/palantir/abc-123",
            "lever",
            "palantir",
        );
    }

    #[test]
    fn personio_positive() {
        // Subdomain slug, lowercased (DNS label).
        assert_ref("https://acme.jobs.personio.de/job/42", "personio", "acme");
        assert_ref("https://globex.jobs.personio.com/", "personio", "globex");
    }

    #[test]
    fn workable_positive() {
        assert_ref(
            "https://apply.workable.com/careers-at-sleek",
            "workable",
            "careers-at-sleek",
        );
        assert_ref(
            "https://apply.workable.com/acme/j/ABCDEF",
            "workable",
            "acme",
        );
    }

    #[test]
    fn ashby_preserves_slug_casing() {
        // Ashby board tokens are case-sensitive — must NOT be lowercased.
        assert_ref("https://jobs.ashbyhq.com/Linear", "ashby", "Linear");
        assert_ref(
            "https://jobs.ashbyhq.com/Perplexity/uuid-1",
            "ashby",
            "Perplexity",
        );
    }

    #[test]
    fn recruitee_positive() {
        assert_ref("https://acme.recruitee.com", "recruitee", "acme");
        assert_ref(
            "https://globex.recruitee.com/o/backend-engineer",
            "recruitee",
            "globex",
        );
    }

    #[test]
    fn smartrecruiters_both_hosts_preserve_casing() {
        assert_ref(
            "https://jobs.smartrecruiters.com/AcmeCorp/12345",
            "smartrecruiters",
            "AcmeCorp",
        );
        assert_ref(
            "https://careers.smartrecruiters.com/Globex",
            "smartrecruiters",
            "Globex",
        );
    }

    #[test]
    fn breezy_positive() {
        assert_ref("https://acme.breezy.hr", "breezy", "acme");
        assert_ref("https://globex.breezy.hr/p/xyz", "breezy", "globex");
    }

    #[test]
    fn bamboohr_positive() {
        assert_ref("https://acme.bamboohr.com/careers/17", "bamboohr", "acme");
    }

    #[test]
    fn pinpoint_positive() {
        assert_ref("https://acme.pinpointhq.com", "pinpoint", "acme");
        assert_ref(
            "https://globex.pinpointhq.com/postings/1",
            "pinpoint",
            "globex",
        );
    }

    #[test]
    fn rippling_positive_preserves_slug_casing() {
        // Posting URLs are host-locked to `ats.rippling.com/{slug}/jobs/{id}` (the
        // exact shape `boards::rippling` emits/guards). Slug = first path segment.
        assert_ref(
            "https://ats.rippling.com/acme/jobs/job-abc-123",
            "rippling",
            "acme",
        );
        // Rippling slugs are URL path segments, not DNS labels — mixed case kept.
        assert_ref(
            "https://ats.rippling.com/Acme-Corp/jobs/x",
            "rippling",
            "Acme-Corp",
        );
    }

    #[test]
    fn rippling_invalid_slug_shape_returns_none() {
        // A first path segment whose SHAPE the board's `is_valid_rippling_slug`
        // rejects (dot, leading/trailing hyphen, underscore, over-length) must not
        // be harvested — the store must never hold a slug the board would refuse.
        for url in [
            "https://ats.rippling.com/acme.corp/jobs/1", // dot
            "https://ats.rippling.com/-acme/jobs/1",     // leading hyphen
            "https://ats.rippling.com/acme-/jobs/1",     // trailing hyphen
            "https://ats.rippling.com/acme_corp/jobs/1", // underscore
        ] {
            assert_eq!(
                extract_ats_ref(url),
                None,
                "an invalid-shape rippling slug must not extract: {url}"
            );
        }
        // A 64-char first segment (board cap is 63) is also refused.
        let too_long = format!("https://ats.rippling.com/{}/jobs/1", "a".repeat(64));
        assert_eq!(extract_ats_ref(&too_long), None, "over-length slug refused");
    }

    // ── Near-miss suite → None ───────────────────────────────────────────────────

    #[test]
    fn near_misses_return_none() {
        for url in [
            // Marketing / non-board greenhouse hosts.
            "https://greenhouse.io/blog/how-to-hire",
            "https://www.greenhouse.io/",
            "https://boards-api.greenhouse.io/v1/boards/stripe/jobs",
            // Bare apex domains (no company subdomain / no path).
            "https://greenhouse.io",
            "https://lever.co",
            "https://recruitee.com",
            "https://breezy.hr",
            "https://bamboohr.com",
            "https://pinpointhq.com",
            "https://workable.com",
            "https://ashbyhq.com",
            "https://smartrecruiters.com",
            "https://jobs.personio.de",
            "https://rippling.com",
            "https://ats.rippling.com", // bare host, no slug path segment
            // `www.` fronts on subdomain ATSes.
            "https://www.recruitee.com",
            "https://www.bamboohr.com",
            // Look-alike suffix-evasion hosts.
            "https://jobs.lever.co.attacker.tld/stripe",
            "https://acme.recruitee.com.attacker.tld",
            "https://jobs.smartrecruiters.com.attacker.tld/Acme/1",
            "https://evilrecruitee.com/acme",
            "https://ats.rippling.com.attacker.tld/acme/jobs/1",
            // Wrong rippling host: `api.rippling.com` is the API (first path
            // segment is `platform`, never a slug), `www.rippling.com` is marketing.
            "https://api.rippling.com/platform/api/ats/v1/board/acme/jobs",
            "https://www.rippling.com/acme",
            // Wrong workable host (only apply.workable.com carries a slug).
            "https://www.workable.com/acme",
            // Completely unrelated hosts.
            "https://example.com/jobs/123",
            "https://linkedin.com/jobs/view/1",
            // Unparseable.
            "not a url at all",
            "",
        ] {
            assert_eq!(
                extract_ats_ref(url),
                None,
                "near-miss URL {url:?} must not extract an ATS ref"
            );
        }
    }

    #[test]
    fn multi_level_subdomains_return_none() {
        // A multi-level subdomain UNDER a real ATS suffix (e.g. a `careers.`/`jobs.`
        // front on top of the company label) is NOT a company host — `subdomain_slug`
        // only accepts the single label directly under the suffix. Pins the
        // `label.contains('.')` branch so a future "take the first label" change can't
        // silently harvest `careers` as the slug from `careers.acme.recruitee.com`.
        for url in [
            "https://careers.acme.recruitee.com/o/backend",
            "https://jobs.acme.breezy.hr/p/xyz",
            "https://careers.acme.bamboohr.com/careers/17",
            "https://jobs.acme.pinpointhq.com/postings/1",
        ] {
            assert_eq!(
                extract_ats_ref(url),
                None,
                "multi-level subdomain {url:?} must not extract a slug"
            );
        }
    }

    #[test]
    fn greenhouse_embed_without_for_is_none() {
        // The embed widget with no `for=` slug is unusable.
        assert_eq!(
            extract_ats_ref("https://boards.greenhouse.io/embed/job_app?token=99"),
            None
        );
    }

    #[test]
    fn host_matching_is_case_insensitive() {
        // Uppercase host must still match (the url crate lowercases it); the slug
        // casing is independent and preserved.
        assert_ref("https://JOBS.ASHBYHQ.COM/Linear", "ashby", "Linear");
    }
}
