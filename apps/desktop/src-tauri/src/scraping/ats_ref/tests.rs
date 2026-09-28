//! Tests for [`super::extract_ats_ref`] and its per-ATS slug parsers.
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
