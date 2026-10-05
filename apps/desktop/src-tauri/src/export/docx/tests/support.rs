//! Shared fixtures + OOXML-part helpers reused across the docx test topics.

use std::io::{Cursor, Read};

use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};

/// Unzip a generated DOCX and return its `word/document.xml` (where the body
/// runs and the section's `pgSz` live).
pub(super) fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx is a zip archive");
    let mut file = zip
        .by_name("word/document.xml")
        .expect("docx contains word/document.xml");
    let mut xml = String::new();
    file.read_to_string(&mut xml).expect("read document.xml");
    xml
}

pub(super) fn resume_request(template_id: TemplateId) -> ExportRequest {
    ExportRequest {
        // Name + contact + section + entry + bullet exercise name/heading/body fonts.
        text: "Jane Doe\njane@example.com\n\nEXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n- Built things that mattered".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    }
}

/// Cover-letter request builder for the letter-layout DOCX tests (PR5).
pub(super) fn letter_request(text: &str, layout: LetterLayout) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: layout,
    }
}

pub(super) const REFINED_US_TEXT: &str = "Jane Smith\njane@example.com | https://linkedin.com/in/janesmith\n\nJune 2, 2025\n\nHiring Manager\nAcme Corp\n\nRe: Application for Platform Engineer (Ref PX-2291)\n\nDear Hiring Manager,\n\nI am writing to express my strong interest in the Platform Engineer position, bringing distributed systems experience.\n\nSincerely,\n\nJane Smith\nSoftware Engineer\n";

pub(super) const REFINED_DE_TEXT: &str = "Max Müller\nmax@example.de | https://linkedin.com/in/maxmueller\n\nFrankfurt, 2. Juni 2025\n\nFrau Dr. Anna Weber\nMusterfirma GmbH\n\nBetreff: Bewerbung als Software Engineer\n\nSehr geehrte Frau Dr. Weber,\n\nmit großem Interesse habe ich Ihre Stellenausschreibung gelesen und bewerbe mich hiermit.\n\nMit freundlichen Grüßen,\n\nMax Müller\n";

/// Body-only fixture — regression guardrail for the shipped
/// `complete_letter_text` fix. Mirrors `LETTER_FIXTURE_BODY_ONLY_US` in
/// `typst_engine/tests/letter_fixtures.rs` (this file's fixtures are one-line `\n`-escaped;
/// duplicated rather than shared because the two test modules have no
/// production-code seam to reach a common fixture without touching
/// non-test code — this file's German fixture already drifts from the PDF
/// engine's own `LETTER_FIXTURE_DE` vs `REFINED_DE_TEXT` above, so keeping
/// this new pair in step across both files rather than trying to unify them
/// matches that existing precedent). No letterhead, no salutation, no
/// sign-off, no signature — the shape `pipeline::resume::prompts::letter_system`
/// actually asks the model for. One `**bold**` keyword.
pub(super) const LETTER_FIXTURE_BODY_ONLY_US: &str = "I am writing to express my strong interest in the Software Engineer position, where I would bring five years of experience building distributed systems in Rust and Go to a team solving problems at real scale.\n\nDuring my time at Beta Inc, I led the migration of our payments service to a **microservices** architecture, reducing end-to-end latency by 40 percent and cutting infrastructure costs by 30 percent.\n\nI would welcome the opportunity to discuss how my background aligns with your team's needs and how I could contribute from day one.";

/// German body-only fixture — mirrors `LETTER_FIXTURE_BODY_ONLY_DE` in
/// `typst_engine/tests/letter_fixtures.rs`: a long opening paragraph, a paragraph with digits
/// and a mid-sentence period ("von 0 % auf 90 %."), and a `**bold**` keyword.
pub(super) const LETTER_FIXTURE_BODY_ONLY_DE: &str = "Mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als Software Engineer gelesen und bin überzeugt, dass meine mehrjährige Erfahrung in der Entwicklung verteilter Systeme genau zu den Anforderungen passt, die Sie beschrieben haben.\n\nIn meiner bisherigen Tätigkeit bei der Beta GmbH konnte ich die Testabdeckung von 0 % auf 90 % steigern. Durch die Einführung von **Jest** und einer durchgängigen CI-Pipeline wurde die Codequalität spürbar besser.\n\nÜber eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen und stehe für Rückfragen jederzeit zur Verfügung.";

/// Every `w:sz w:val="N"` (half-points) found in a DOCX body, in document order.
/// Deliberately does not match `w:szCs` (the companion complex-script size,
/// same value) — the literal `w:sz w:val="` substring requires a space right
/// after `sz`, which `szCs` never has.
pub(super) fn all_font_sizes(xml: &str) -> Vec<u32> {
    let needle = "w:sz w:val=\"";
    let mut sizes = Vec::new();
    let mut rest = xml;
    while let Some(idx) = rest.find(needle) {
        let after = &rest[idx + needle.len()..];
        let end = after
            .find('"')
            .expect("w:sz w:val opening quote must close");
        sizes.push(
            after[..end]
                .parse::<u32>()
                .expect("w:sz w:val must be numeric"),
        );
        rest = &after[end..];
    }
    sizes
}

/// Split `word/document.xml` into one slice per real `<w:p …>` paragraph
/// element. NOT `xml.split("<w:p>")` (the pattern the Monogram test uses):
/// every paragraph this crate's docx-rs version emits carries a
/// `w14:paraId="…"` attribute (`<w:p w14:paraId="00000001">`), so the bare
/// `"<w:p>"` literal never actually occurs and that split silently returns
/// the WHOLE document as one chunk — harmless for a test that only reads
/// runs out of the single resulting chunk, but useless for telling two
/// paragraphs apart. Matches on the character immediately after `<w:p` being
/// `>` or a space, which is true for a real paragraph tag but false for
/// `<w:pPr>`/`<w:pStyle …>` (`P`/`S` follow directly with no separator).
pub(super) fn docx_paragraphs(xml: &str) -> Vec<&str> {
    let bytes = xml.as_bytes();
    let marker = b"<w:p";
    let mut starts: Vec<usize> = bytes
        .windows(marker.len() + 1)
        .enumerate()
        .filter_map(|(i, w)| {
            (&w[..marker.len()] == marker && (w[marker.len()] == b'>' || w[marker.len()] == b' '))
                .then_some(i)
        })
        .collect();
    starts.push(xml.len());
    starts.windows(2).map(|w| &xml[w[0]..w[1]]).collect()
}
