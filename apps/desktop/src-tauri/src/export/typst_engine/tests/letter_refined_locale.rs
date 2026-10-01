//! Letter Refined layout locale (DE/IT) + subject-line behavior tests.

use super::fixtures::{normalize_like_validator, signature_block, NO_EXTRACTABLE_TEXT_THRESHOLD};
use super::letter_fixtures::{LETTER_FIXTURE_DE, LETTER_FIXTURE_IT, LETTER_FIXTURE_US_SUBJECT};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::render_letter_pdf;

// (R2) Refined honours DE DIN conventions: A4, Betreff subject present, German
// salutation + sign-off, and the DIN top-right date reads before the salutation.
#[test]
fn letter_refined_de_honors_din_conventions() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined DE render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined DE must start with %PDF"
    );

    let normalised: String = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined DE")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let lower = normalised.to_lowercase();

    // The subject text survives (rendered as the job-reference line). The
    // "Betreff" label prefix is stripped, so assert on the subject body.
    assert!(
        lower.contains("bewerbung"),
        "DE subject body missing:\n{lower}"
    );
    assert!(
        lower.contains("sehr geehr"),
        "German salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("freundlichen"),
        "German sign-off missing:\n{lower}"
    );
    // Signature name — previously unasserted here, leaving the Refined layout
    // with zero extraction coverage of the candidate's own name.
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "refined DE: signature name missing\n---\n{lower}"
    );

    // DIN date-top-right → the date reads near the top, before the salutation.
    let pos_date = lower.find("2025").expect("date present");
    let pos_sal = lower.find("sehr geehr").expect("salutation present");
    assert!(
        pos_date < pos_sal,
        "DE DIN date should precede the salutation — date={pos_date} sal={pos_sal}"
    );
}

// (R2b) Refined round-trips accented-Latin content — grave lowercase + capital
// È/À (see `ACCENTED_RESUME_FIXTURE`/`LETTER_FIXTURE_IT` doc comments for
// rationale). Complements (R2) above, which only exercises German ü/ß.
#[test]
fn letter_refined_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "refined: accented signature name missing — capitals È/À did not survive extraction\n---\n{extracted}"
    );
    // The name appears TWICE — letterhead and signature — so the global
    // `contains` above still passes if extraction drops the whole sign-off
    // block. Pin the signature itself by looking only after the sign-off.
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "refined: accented name missing from the SIGNATURE (after the sign-off) — a \
         letterhead-only match would hide a dropped signature\n---\n{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "refined: grave-accented-lowercase body word missing\n---\n{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "refined accented-Latin: only {normalized_len} normalized chars extracted — \
         the real validator's no_extractable_text gate would block this export"
    );
}

// (R3) Refined always shows a subject as the JOB REFERENCE line — even when the
// market omits the subject (US `subject_line_used = false`). Classic drops it.
#[test]
fn letter_refined_shows_subject_when_market_omits_it() {
    let t = Template::get(TemplateId::SwissMinimal);

    let refined = render_letter_pdf(
        LETTER_FIXTURE_US_SUBJECT,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined US-subject render");
    let refined_txt = pdf_extract::extract_text_from_mem(&refined)
        .expect("pdf-extract refined")
        .to_lowercase();
    assert!(
        refined_txt.contains("px-2291"),
        "Refined must render the subject reference (px-2291):\n{refined_txt}"
    );

    let classic = render_letter_pdf(
        LETTER_FIXTURE_US_SUBJECT,
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
    .expect("classic US-subject render");
    let classic_txt = pdf_extract::extract_text_from_mem(&classic)
        .expect("pdf-extract classic")
        .to_lowercase();
    assert!(
        !classic_txt.contains("px-2291"),
        "Classic must NOT render the subject when the market omits it (subject_line_used=false):\n{classic_txt}"
    );
}
