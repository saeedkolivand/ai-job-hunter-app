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

#[test]
fn fill_empty_from_seeds_an_empty_full_name_but_never_overwrites_one() {
    let suggested = ContactProfile {
        full_name: Some("Jane Doe".into()),
        ..Default::default()
    };
    let mut empty = ContactProfile::default();
    empty.fill_empty_from(&suggested);
    assert_eq!(empty.full_name.as_deref(), Some("Jane Doe"));

    let mut set = ContactProfile {
        full_name: Some("J. Doe".into()),
        ..Default::default()
    };
    set.fill_empty_from(&suggested);
    assert_eq!(set.full_name.as_deref(), Some("J. Doe"));
}

#[test]
fn linkedin_from_text_linkifies_only_a_schemeless_linkedin_in_profile() {
    assert_eq!(
        linkedin_from_text("Jane Doe\njane@x.io | linkedin.com/in/jane-doe | Berlin").as_deref(),
        Some("https://linkedin.com/in/jane-doe")
    );
    assert_eq!(
        linkedin_from_text("www.linkedin.com/in/jane-doe").as_deref(),
        Some("https://www.linkedin.com/in/jane-doe")
    );
    for no in [
        "linkedin.com/company/acme",
        "example.com/in/jane",
        "notlinkedin.com/in/jane",
        "https://www.linkedin.com/in/jane",
    ] {
        assert_eq!(linkedin_from_text(no), None, "{no}");
    }
}

#[test]
fn linkedin_from_text_keeps_unicode_slugs_and_skips_a_pdf_wrapped_url() {
    assert_eq!(
        linkedin_from_text("linkedin.com/in/jürgen-müller | Berlin").as_deref(),
        Some("https://linkedin.com/in/jürgen-müller")
    );
    // Soft-wrapped mid-slug: the head alone would be a truncated URL.
    assert_eq!(linkedin_from_text("linkedin.com/in/jane-\ndoe"), None);
}
