//! Letter Banded and Navy layout render tests.

use super::fixtures::{normalize_like_validator, signature_block, NO_EXTRACTABLE_TEXT_THRESHOLD};
use super::letter_fixtures::{LETTER_FIXTURE_DE, LETTER_FIXTURE_IT, LETTER_FIXTURE_US};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::render_letter_pdf;

// (B1) Banded layout renders a valid US PDF with correct reading order.
#[test]
fn letter_banded_us_renders_valid_pdf() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded US render should succeed");
    assert!(bytes.starts_with(b"%PDF"), "banded US must start with %PDF");

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded US")
        .to_lowercase();
    assert!(
        lower.contains("dear hiring manager"),
        "salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("distributed systems"),
        "body phrase missing:\n{lower}"
    );
    assert!(lower.contains("sincerely"), "sign-off missing:\n{lower}");

    let pos_sal = lower.find("dear").expect("salutation present");
    let pos_body = lower.find("distributed").expect("body present");
    let pos_signoff = lower.find("sincerely").expect("sign-off present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "banded US reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient inside-address renders (unconditional in Banded) — pin it so a
    // future refactor can't regress it the way Classic silently did.
    assert!(
        lower.contains("123 main street"),
        "banded US: recipient inside address missing:\n{lower}"
    );
    assert!(
        lower.find("acme corp").is_some_and(|p| p < pos_sal),
        "banded US: inside address must read before the salutation:\n{lower}"
    );
}

// (B2) Banded honours DE conventions: A4, Betreff subject (subject_line_used),
// German salutation + sign-off.
#[test]
fn letter_banded_de_honors_din_subject() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded DE render should succeed");
    assert!(bytes.starts_with(b"%PDF"), "banded DE must start with %PDF");

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded DE")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        lower.contains("betreff"),
        "DE Betreff subject missing:\n{lower}"
    );
    assert!(
        lower.contains("sehr geehr"),
        "German salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("freundlichen"),
        "German sign-off missing:\n{lower}"
    );
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "banded DE: signature name missing\n---\n{lower}"
    );
}

// (B2b) Banded round-trips accented-Latin content — grave lowercase + capital
// È/À (see `ACCENTED_RESUME_FIXTURE`/`LETTER_FIXTURE_IT` doc comments for
// rationale). Complements (B2) above, which only exercises German ü/ß.
#[test]
fn letter_banded_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "banded accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "banded: accented signature name missing — capitals È/À did not survive extraction\n---\n{extracted}"
    );
    // Same letterhead-vs-signature distinction as the refined case above.
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "banded: accented name missing from the SIGNATURE (after the sign-off) — a \
         letterhead-only match would hide a dropped signature\n---\n{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "banded: grave-accented-lowercase body word missing\n---\n{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "banded accented-Latin: only {normalized_len} normalized chars extracted — \
         the real validator's no_extractable_text gate would block this export"
    );
}

/// The Navy letter layout must extract like its siblings. This is the test that
/// caught Cologne Navy's tracking: at the design brief's 0.14em the NAME came
/// back as "À LVA R O   È S P O S I T O" — unreadable to an ATS. The letterhead
/// carries the same tracked-caps treatment, so it needs the same guard.
#[test]
fn letter_navy_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::CologneNavy);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Navy,
            ats: false,
        },
    )
    .expect("navy accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "navy accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on navy accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "navy: accented name missing — capitals È/À did not survive extraction
---
{extracted}"
    );
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "navy: accented name missing from the SIGNATURE (after the sign-off) — a          letterhead-only match would hide a dropped signature
---
{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "navy: grave-accented-lowercase body word missing
---
{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "navy accented-Latin: only {normalized_len} normalized chars extracted —          the real validator's no_extractable_text gate would block this export"
    );
}
