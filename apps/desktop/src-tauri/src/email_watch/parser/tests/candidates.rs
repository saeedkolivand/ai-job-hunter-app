use super::*;

// ── extract_candidates ───────────────────────────────────────────────────

/// The company candidate from a subject, a body snippet, or the sender's display name (tried in
/// that order) — including company names that legitimately contain `and`/`und`, which must not be
/// truncated at the first one (item 10 regression). `(subject, body, sender, expected company)`.
#[test]
fn extracts_the_company_from_subject_body_or_sender() {
    type Case = (
        &'static str,
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
    );
    let cases: &[Case] = &[
        (
            "Your application to Acme Corp was received",
            None,
            None,
            Some("Acme Corp"),
        ),
        (
            "Ihre Bewerbung bei Acme GmbH ist eingegangen",
            None,
            None,
            Some("Acme GmbH"),
        ),
        (
            "Application received",
            Some("Thanks for applying! Your application to Acme Corp is being reviewed."),
            None,
            Some("Acme Corp"),
        ),
        (
            "Application received",
            None,
            Some("Acme Corp Careers"),
            Some("Acme Corp"),
        ),
        ("Application received", None, Some("no-reply"), None),
        (
            "Your application to Johnson and Johnson was received",
            None,
            None,
            Some("Johnson and Johnson"),
        ),
        (
            "Ihre Bewerbung bei Miller und Frost ist eingegangen",
            None,
            None,
            Some("Miller und Frost"),
        ),
    ];
    for &(subject, body, sender, expected) in cases {
        let c = extract_candidates(subject, body, sender);
        assert_eq!(c.company.as_deref(), expected, "{subject:?}");
    }
}

/// A title is extracted alongside the company when the subject carries both (EN, then DE).
#[test]
fn extracts_title_and_company_from_a_subject() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "Thank you for applying for the Software Engineer position at Acme Corp!",
            "Acme Corp",
            "Software Engineer",
        ),
        (
            "Ihre Bewerbung als Software Engineer bei Acme GmbH",
            "Acme GmbH",
            "Software Engineer",
        ),
    ];
    for &(subject, company, title) in cases {
        let c = extract_candidates(subject, None, None);
        assert_eq!(c.company.as_deref(), Some(company), "{subject:?}");
        assert_eq!(c.title.as_deref(), Some(title), "{subject:?}");
    }
}

#[test]
fn extract_candidates_is_none_when_nothing_matches_anywhere() {
    let c = extract_candidates("Application received", Some("Please wait."), None);
    assert_eq!(c.company, None);
    assert_eq!(c.title, None);
}

// ── parse_header / parse_body_text ──────────────────────────────────────

#[test]
fn parse_header_decodes_rfc2047_subject_and_lowercases_the_domain() {
    let raw = b"From: Acme Careers <Careers@ACME.example.com>\r\n\
Subject: =?UTF-8?B?VGhhbmsgeW91IGZvciBhcHBseWluZyE=?=\r\n\
Message-ID: <abc123@example.com>\r\n\
\r\n";
    let header = parse_header(raw).expect("should parse a minimal header block");
    assert_eq!(header.subject, "Thank you for applying!");
    assert_eq!(header.from_name.as_deref(), Some("Acme Careers"));
    assert_eq!(header.from_domain.as_deref(), Some("acme.example.com"));
    assert_eq!(header.message_id.as_deref(), Some("abc123@example.com"));
}

#[test]
fn parse_body_text_extracts_plain_text() {
    let raw = b"From: Acme <careers@acme.example.com>\r\n\
Subject: Your application to Acme Corp\r\n\
Content-Type: text/plain; charset=\"us-ascii\"\r\n\
\r\n\
Thanks for applying to Acme Corp!\r\n";
    let text = parse_body_text(raw).expect("should parse a plain-text body");
    assert!(text.contains("Thanks for applying to Acme Corp!"));
}
