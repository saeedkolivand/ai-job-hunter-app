use super::*;
use crate::extraction::types::Link;

fn link(url: &str) -> Link {
    Link {
        anchor_text: url.to_string(),
        url: url.to_string(),
    }
}

#[test]
fn header_markdown_uses_named_fields_in_canonical_order() {
    let p = ContactProfile {
        full_name: Some("Alex Carter".into()),
        email: Some("alex.carter@example.com".into()),
        phone: Some("+31 6 12345678".into()),
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: [("de".to_string(), "Niederlande".to_string())].into(),
        }),
        linkedin: Some("https://www.linkedin.com/in/alex-carter/".into()),
        github: Some("https://github.com/alexcarter".into()),
        website: Some("https://solo.to/alexc".into()),
        extra_links: vec![],
        photo: None,
    };

    // German doc: localized location, canonical order.
    assert_eq!(
        p.header_markdown("de"),
        "Niederlande | alex.carter@example.com | +31 6 12345678 | \
         [LinkedIn](https://www.linkedin.com/in/alex-carter/) | \
         [GitHub](https://github.com/alexcarter) | [Website](https://solo.to/alexc)"
    );
    // English doc: default location.
    assert!(p.header_markdown("en").starts_with("Netherlands | "));
}

#[test]
fn header_rich_makes_each_named_link_clickable_with_the_right_url() {
    let p = ContactProfile {
        email: Some("alex.carter@example.com".into()),
        linkedin: Some("https://www.linkedin.com/in/alex-carter/".into()),
        github: Some("https://github.com/alexcarter".into()),
        website: Some("https://solo.to/alexc".into()),
        ..Default::default()
    };
    let rich = p.header_rich("en");
    // The LinkedIn label is bound to the PERSONAL profile URL (not a company page).
    let linkedin = rich
        .iter()
        .find(|r| r.text == "LinkedIn")
        .expect("LinkedIn run");
    assert_eq!(
        linkedin.link.as_deref(),
        Some("https://www.linkedin.com/in/alex-carter/")
    );
    let website = rich.iter().find(|r| r.text == "Website").expect("Website");
    assert_eq!(website.link.as_deref(), Some("https://solo.to/alexc"));
    assert!(rich
        .iter()
        .any(|r| r.link.as_deref() == Some("mailto:alex.carter@example.com")));
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
fn empty_profile_is_detected() {
    assert!(ContactProfile::default().is_effectively_empty());
    let only_name = ContactProfile {
        full_name: Some("x".into()),
        ..Default::default()
    };
    assert!(
        only_name.is_effectively_empty(),
        "name alone is not a header"
    );
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

#[test]
fn fill_empty_from_completes_sparse_profile_without_clobbering() {
    // The user edited only the website (their portfolio); an import suggests a full set.
    let mut current = ContactProfile {
        website: Some("https://my.portfolio/".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("l@example.com".into()),
        phone: Some("+31 6 12345678".into()),
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".into(),
            ..Default::default()
        }),
        linkedin: Some("https://www.linkedin.com/in/l/".into()),
        website: Some("https://drive.google.com/xyz".into()), // must NOT overwrite the user's
        extra_links: vec![ContactLink {
            label: "Dribbble".into(),
            url: "https://dribbble.com/l".into(),
        }],
        ..Default::default()
    };
    current.fill_empty_from(&suggested);

    assert_eq!(
        current.website.as_deref(),
        Some("https://my.portfolio/"),
        "a user-set field is never overwritten"
    );
    assert_eq!(current.email.as_deref(), Some("l@example.com"));
    assert_eq!(current.phone.as_deref(), Some("+31 6 12345678"));
    assert_eq!(
        current.location.as_ref().map(|l| l.default.as_str()),
        Some("Amsterdam, Netherlands")
    );
    assert_eq!(
        current.linkedin.as_deref(),
        Some("https://www.linkedin.com/in/l/")
    );
    assert!(current.extra_links.iter().any(|e| e.label == "Dribbble"));
}

#[test]
fn fill_empty_from_merges_extras_by_url_without_duplicates() {
    let mut current = ContactProfile {
        extra_links: vec![ContactLink {
            label: "Dribbble".into(),
            url: "https://dribbble.com/l".into(),
        }],
        ..Default::default()
    };
    let suggested = ContactProfile {
        extra_links: vec![
            ContactLink {
                label: "Dribbble".into(),
                url: "https://dribbble.com/l".into(), // duplicate by URL → skipped
            },
            ContactLink {
                label: "Behance".into(),
                url: "https://behance.net/l".into(),
            },
        ],
        ..Default::default()
    };
    current.fill_empty_from(&suggested);
    assert_eq!(
        current.extra_links.len(),
        2,
        "duplicate deduped, new extra added: {:?}",
        current.extra_links
    );
}

#[test]
fn localized_text_resolves_primary_subtag() {
    let loc = LocalizedText {
        default: "Netherlands".into(),
        by_lang: [("de".to_string(), "Niederlande".to_string())].into(),
    };
    assert_eq!(loc.resolve("de"), "Niederlande");
    assert_eq!(loc.resolve("de-DE"), "Niederlande");
    assert_eq!(loc.resolve("en"), "Netherlands");
    assert_eq!(loc.resolve("fr"), "Netherlands");
}
