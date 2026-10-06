use super::*;

fn link(url: &str) -> Link {
    Link {
        anchor_text: url.to_string(),
        url: url.to_string(),
    }
}

#[test]
fn classify_picks_personal_links_and_rejects_company_pool() {
    // Mirrors the bug data set: a personal profile, a company page, an employer
    // site, plus the real personal site — in document order.
    let links = vec![
        link("https://www.linkedin.com/in/alex-carter/"),
        link("https://github.com/alexcarter"),
        link("https://www.linkedin.com/company/acme/about/"),
        link("http://example-employer.com"),
        link("https://solo.to/alexc"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.linkedin.as_deref(),
        Some("https://www.linkedin.com/in/alex-carter/"),
        "must pick the personal /in/ profile, never the /company/ page"
    );
    assert_eq!(p.github.as_deref(), Some("https://github.com/alexcarter"));
    assert_eq!(
        p.website.as_deref(),
        Some("https://solo.to/alexc"),
        "a known link-in-bio host wins the Website slot over an employer URL"
    );
}

#[test]
fn classify_does_not_use_a_job_board_as_website() {
    let links = vec![
        link("https://www.indeed.com/cmp/acme"),
        link("https://my-portfolio.dev"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.website.as_deref(),
        Some("https://my-portfolio.dev"),
        "job-board URL must be skipped; the real portfolio takes Website"
    );
}

#[test]
fn classify_extracts_mailto_email() {
    let links = vec![link("mailto:alex.carter@example.com")];
    let p = classify_contact_links(&links);
    assert_eq!(p.email.as_deref(), Some("alex.carter@example.com"));
}

#[test]
fn classify_keeps_other_personal_links_as_labelled_extras() {
    // A personal profile + a known website host + two portfolio links + a job board.
    let links = vec![
        link("https://www.linkedin.com/in/lena-vos/"),
        link("https://solo.to/lenavos"), // website-host → Website slot
        link("https://dribbble.com/lenavos"),
        link("https://www.behance.net/lenavos"),
        link("https://www.indeed.com/cmp/acme"), // job board → never surfaced
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.website.as_deref(), Some("https://solo.to/lenavos"));

    let labels: Vec<&str> = p.extra_links.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Dribbble"), "extras = {labels:?}");
    assert!(labels.contains(&"Behance"), "extras = {labels:?}");
    assert!(
        !p.extra_links.iter().any(|e| e.url.contains("linkedin.com")
            || e.url.contains("solo.to")
            || e.url.contains("indeed.com")),
        "named fields and job boards must not leak into extras: {:?}",
        p.extra_links
    );
}

/// Project/repo/demo links must never leak into the contact profile, even
/// though they share a host with a genuine platform profile — only the
/// profile-shaped form (bare user page) qualifies. Mirrors `isProfileShaped`/
/// `classifyLinks` in `packages/prompts/src/generate/links/classify.ts`.
#[test]
fn classify_excludes_deep_path_project_links_by_shape() {
    let links = vec![
        link("https://github.com/alice"),
        link("https://github.com/alice/my-project"),
        link("https://gitlab.com/alice"),
        link("https://gitlab.com/alice/my-project"),
        link("https://linkedin.com/in/alice"),
        link("https://linkedin.com/company/acme"),
        link("https://myapp.com/demo"),
        link("https://alice.dev"),
        link("https://dribbble.com/alice"),
    ];
    let p = classify_contact_links(&links);

    assert_eq!(p.github.as_deref(), Some("https://github.com/alice"));
    assert_eq!(p.linkedin.as_deref(), Some("https://linkedin.com/in/alice"));
    assert_eq!(p.website.as_deref(), Some("https://alice.dev"));

    let extra_urls: Vec<&str> = p.extra_links.iter().map(|e| e.url.as_str()).collect();
    assert!(
        extra_urls.contains(&"https://dribbble.com/alice"),
        "a platform profile must still seed extras: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://github.com/alice"),
        "a github profile promoted to the github field must not also appear in extras: {extra_urls:?}"
    );
    assert!(
        extra_urls.contains(&"https://gitlab.com/alice"),
        "a bare GitLab profile is profile-shaped and must seed extras: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://github.com/alice/my-project"),
        "a repo URL is a project reference, not an identity — must not leak: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://gitlab.com/alice/my-project"),
        "a GitLab repo URL is a project reference, not an identity — must not leak: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://linkedin.com/company/acme"),
        "a company page must never seed the header: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://myapp.com/demo"),
        "a deep-path demo link must never seed the header: {extra_urls:?}"
    );
}

/// The bug data set: an apex domain, one of its own subdomains, an unrelated
/// bare-root project domain, and a GitHub user + one of that user's repos.
/// Only the apex reaches `website`; the unrelated bare-root domain and the
/// subdomain never enter the profile anywhere (not `website`, not
/// `extra_links`) since a personal bare-root domain that loses the website
/// slot is a body link, never a header link.
#[test]
fn website_prefers_apex_over_subdomain_and_rejected_domains_never_leak_to_extras() {
    let links = vec![
        link("https://apex.dev"),
        link("https://sub.apex.dev"),
        link("https://other.app"),
        link("https://github.com/u"),
        link("https://github.com/u/repo"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.website.as_deref(), Some("https://apex.dev"));
    assert_eq!(p.github.as_deref(), Some("https://github.com/u"));
    assert!(
        p.extra_links.is_empty(),
        "a rejected bare-root domain (sub.apex.dev, other.app) or a repo path \
         (github.com/u/repo) must never leak into extra_links: {:?}",
        p.extra_links
    );
}

/// The SAME input, order reversed — proves website selection is
/// order-independent: the apex/subdomain shape relationship decides the
/// winner, not raw document position.
#[test]
fn website_prefers_apex_over_subdomain_order_independent() {
    let links = vec![
        link("https://github.com/u/repo"),
        link("https://github.com/u"),
        link("https://other.app"),
        link("https://sub.apex.dev"),
        link("https://apex.dev"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.website.as_deref(),
        Some("https://apex.dev"),
        "apex must win regardless of document order"
    );
    assert_eq!(p.github.as_deref(), Some("https://github.com/u"));
    assert!(p.extra_links.is_empty(), "{:?}", p.extra_links);
}

/// A second GitHub user (not just a repo path under the first) is still a
/// genuine platform profile and must still surface as an extra — the
/// extras-are-platform-profiles-only tightening must not regress this
/// documented intent (same for Dribbble / Behance).
#[test]
fn second_platform_profile_still_becomes_extra_after_extras_tightening() {
    let links = vec![
        link("https://github.com/alice"),
        link("https://github.com/bob"),
        link("https://dribbble.com/alice"),
        link("https://www.behance.net/alice"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.github.as_deref(), Some("https://github.com/alice"));
    let extra_urls: Vec<&str> = p.extra_links.iter().map(|e| e.url.as_str()).collect();
    assert!(
        extra_urls.contains(&"https://github.com/bob"),
        "a second GitHub user must still become an extra: {extra_urls:?}"
    );
    let labels: Vec<&str> = p.extra_links.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Dribbble"), "extras = {labels:?}");
    assert!(labels.contains(&"Behance"), "extras = {labels:?}");
}
