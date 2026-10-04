use crate::export::parser::{
    is_contact_shaped, is_first_line_contact_shaped, parse_line, parse_resume, strip_md,
};
use crate::export::types::LineKind;

#[test]
fn test_contact_detection() {
    let line = parse_line("john@example.com", 5, &[]);
    assert!(matches!(line.kind, LineKind::Contact));
}

#[test]
fn is_contact_shaped_matches_ts_is_header_contact_line_fixture() {
    // Cross-language parity guard: this exact fixture is also asserted by the TS
    // isHeaderContactLine() / isFirstLineContactShaped() tests in
    // packages/prompts/src/generate/text/header-contact-line.test.ts. Both read
    // the same file, so the two implementations can never silently drift — see
    // docs/knowledge (item H, header-seeding) for why this matters: a divergence
    // here either lets a leaked link survive re-seeding unrecognised, or
    // duplicates the seeded line on regeneration.
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        contact: bool,
        #[serde(rename = "firstLine")]
        first_line: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/header-contact-line.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read header-contact-line parity fixture \
         (packages/prompts/src/fixtures/header-contact-line.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse header-contact-line parity fixture");

    assert!(
        !cases.is_empty(),
        "header-contact-line parity fixture must not be empty"
    );
    for c in &cases {
        // `parse_line` never calls `is_contact_shaped` on the raw line — only
        // on `clean = strip_md(trimmed)`. Doing the same here is what makes
        // this a real end-to-end parity check against a `**bold**`- or
        // `#`-decorated line, not just an isolated-function coincidence: the
        // TS mirror applies its own equivalent stripping internally now too.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_contact_shaped(&clean),
            c.contact,
            "is_contact_shaped drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
        assert_eq!(
            is_first_line_contact_shaped(&clean),
            c.first_line,
            "is_first_line_contact_shaped drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

/// A contact line carrying a phone + a bare year but NO email must stay Contact —
/// the `@`-only guard was insufficient (real contacts have a phone, not an email).
#[test]
fn contact_line_phone_and_year_stays_contact() {
    let line = parse_line("Berlin, Germany | +49 30 1234567 | 2021", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::JobEntry),
        "phone+year contact must NOT be JobEntry, got {:?}",
        line.kind
    );
}

/// Contact line with email MUST still be Contact even if it has pipes.
/// "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn" → Contact
#[test]
fn contact_line_with_email_stays_contact() {
    let line = parse_line(
        "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (has '@'), got {:?}",
        line.kind
    );
}

/// Contact line with only pipes and no date MUST still be Contact.
/// "New York | LinkedIn | github.com/jane" → Contact (URL_RE matches)
#[test]
fn contact_line_pipes_no_date_stays_contact() {
    let line = parse_line("New York | linkedin.com/in/jane | github.com/jane", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (URL match), got {:?}",
        line.kind
    );
}

/// A contact line must never be claimed as a job-entry title.
///
/// `is_entry_title_shaped` is checked BEFORE the `is_contact_shaped` branch, so
/// without its own guard a header contact line that happens to be followed by a
/// leading-date line is swallowed into a fabricated entry — the contact details
/// vanish from the header and reappear as a job title.
#[test]
fn a_contact_line_is_never_claimed_as_an_entry_title() {
    let resume = "\
Max Mustermann
Köln, Deutschland · max@example.de · 0179 1402319
Jan 2021 – Heute, Berlin
- Ein Bulletpoint
";
    let parsed = parse_resume(resume);
    let contact = parsed
        .lines
        .iter()
        .find(|l| l.text.contains("max@example.de"))
        .expect("the contact line must still be present");
    assert_ne!(
        contact.kind,
        LineKind::JobEntry,
        "a contact line was turned into a job entry: {contact:?}"
    );
}
