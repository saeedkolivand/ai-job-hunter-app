use super::*;

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
