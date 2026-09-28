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
mod tests;
