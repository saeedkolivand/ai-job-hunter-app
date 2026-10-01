//! Phase 1c cover-letter render tests (Classic layout, US/DE).

use super::letter_fixtures::{LETTER_FIXTURE_DE, LETTER_FIXTURE_US};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::render_letter_pdf;

// (1) US letter renders to a valid PDF and contains expected text.
#[test]
fn letter_us_renders_valid_pdf_with_expected_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us) should succeed");

    assert!(!bytes.is_empty(), "US letter PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "US letter output must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on US letter output");
    let lower = extracted.to_lowercase();

    // Salutation must be present.
    assert!(
        lower.contains("dear hiring manager"),
        "US letter: salutation 'Dear Hiring Manager' missing\n---\n{extracted}"
    );

    // A body phrase must survive.
    assert!(
        lower.contains("distributed systems"),
        "US letter: body phrase 'distributed systems' missing\n---\n{extracted}"
    );

    // Sign-off must be present.
    assert!(
        lower.contains("sincerely"),
        "US letter: sign-off 'Sincerely' missing\n---\n{extracted}"
    );

    // Signature name.
    assert!(
        lower.contains("jane smith"),
        "US letter: signature name 'Jane Smith' missing\n---\n{extracted}"
    );

    // Ordering: salutation before body before sign-off.
    let pos_sal = lower.find("dear").expect("salutation must be present");
    let pos_body = lower
        .find("distributed")
        .expect("body phrase must be present");
    let pos_signoff = lower.find("sincerely").expect("sign-off must be present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "US letter: reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient / inside-address block — the day-one Classic bug dropped it
    // entirely (fixture `recipientPosition: "left"` matched neither the old
    // "after-date" nor "" gate). The street line is unique to the recipient,
    // and the FIRST "Acme Corp" (the body also names it) must read before the
    // salutation, proving the inside address renders in the right slot.
    assert!(
        lower.contains("123 main street"),
        "US letter: recipient street '123 Main Street' missing — inside address dropped\n---\n{extracted}"
    );
    let pos_recipient = lower
        .find("acme corp")
        .expect("recipient company must be present");
    assert!(
        pos_recipient < pos_sal,
        "US letter: inside address must read before the salutation — recipient={pos_recipient} sal={pos_sal}"
    );
}

// (2) DE letter renders to a valid PDF and contains DIN subject + German conventions.
#[test]
fn letter_de_renders_valid_pdf_with_subject_line() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de) should succeed");

    assert!(!bytes.is_empty(), "DE letter PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "DE letter output must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on DE letter output");

    // Normalise whitespace (Typst can wrap long lines).
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Subject label "Betreff" must be present.
    assert!(
        lower.contains("betreff"),
        "DE letter: subject label 'Betreff' missing\n---\n{lower}"
    );

    // German salutation.
    assert!(
        lower.contains("sehr geehr"),
        "DE letter: German salutation 'Sehr geehr...' missing\n---\n{lower}"
    );

    // Body phrase.
    assert!(
        lower.contains("verteilter systeme") || lower.contains("verteilter"),
        "DE letter: body phrase missing\n---\n{lower}"
    );

    // German sign-off.
    assert!(
        lower.contains("freundlichen"),
        "DE letter: German sign-off missing\n---\n{lower}"
    );

    // Signature name.
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "DE letter: signature name missing\n---\n{lower}"
    );

    // Recipient / inside-address block (Anschriftfeld) — DIN 5008 makes it
    // MANDATORY, yet the day-one Classic bug dropped it entirely. Company +
    // recipient name are unique to the inside address (the body names "Beta
    // GmbH", the salutation only "Weber"), and both must read before the
    // Betreff subject line.
    assert!(
        lower.contains("musterfirma gmbh"),
        "DE letter: recipient company 'Musterfirma GmbH' missing — Anschriftfeld dropped\n---\n{lower}"
    );
    assert!(
        lower.contains("anna weber"),
        "DE letter: recipient name 'Anna Weber' missing\n---\n{lower}"
    );
    let pos_recipient = lower
        .find("musterfirma gmbh")
        .expect("recipient company must be present");
    let pos_subject = lower.find("betreff").expect("subject must be present");
    assert!(
        pos_recipient < pos_subject,
        "DE letter: Anschriftfeld must read before the Betreff line — recipient={pos_recipient} subject={pos_subject}"
    );
}

// (3) Both outputs start with %PDF — belt-and-suspenders after the content tests
// above already assert this; kept as a quick standalone guard.
#[test]
fn letter_us_and_de_both_start_with_pdf_header() {
    let t = Template::get(TemplateId::SwissMinimal);
    let us = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us)");
    let de = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de)");
    assert!(us.starts_with(b"%PDF"), "US letter must start with %PDF");
    assert!(de.starts_with(b"%PDF"), "DE letter must start with %PDF");
}

// (4a) Write the US letter sample to target/ for human eyeballing.
// Informational; always passes; .ok()-style write.
#[test]
fn letter_us_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("letter_us_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("letter_us_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("US letter sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "letter_us_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// (4b) Write the DE letter sample to target/ for human eyeballing.
// Informational; always passes; .ok()-style write.
#[test]
fn letter_de_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("letter_de_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("letter_de_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("DE letter sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "letter_de_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}
