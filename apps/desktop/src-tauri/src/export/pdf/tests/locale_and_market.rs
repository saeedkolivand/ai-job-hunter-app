use crate::export::pdf::generate_pdf;
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

/// Regression: a region-tagged locale (`de-DE`) reaching `ExportRequest` must
/// still resolve to the German `Lebenslauf` section order (Certifications
/// before Skills), not silently fall to the default order because
/// `locale::resume::section_order_for`'s alias arm only matches the bare
/// `"de"`/`"at"`/`"ch"`/`"dach"` tokens. `generate_pdf` canonicalises through
/// `LocaleProfile::get` before `linearize` sees it.
#[test]
fn ats_mode_resolves_region_tagged_german_locale_to_the_de_order() {
    let text = "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
Did things

EDUCATION
Some University  2015 - 2019

CERTIFICATIONS
AWS Certified

SKILLS
Python, Rust";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        // `target_language` set explicitly so `target_lang()` (a SEPARATE
        // concern from this test — the document-content language) doesn't
        // fall back to the raw region-tagged `locale` string, which Typst's
        // `#set text(lang: ...)` rejects (it wants a bare 2/3-letter code).
        meta: Some(GenerationMeta {
            candidate_name: None,
            job_title: None,
            company_name: None,
            target_language: Some("de".to_string()),
        }),
        ats_mode: true,
        locale: Some("de-DE".to_string()),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("ats-mode german resume pdf");
    let rendered = pdf_extract::extract_text_from_mem(&bytes)
        .expect("extract text")
        .to_lowercase();
    let cert = rendered
        .find("certifications")
        .expect("certifications section present");
    let skills = rendered.find("skills").expect("skills section present");
    assert!(
        cert < skills,
        "de-DE must resolve to the DE_ORDER (certifications before skills), \
         not the default order — rendered order was skills-then-certifications: {rendered}"
    );
}

#[test]
fn test_generate_pdf_cover_letter_german_market_with_betreff() {
    // German market: a bold Betreff line + a German salutation + formal sign-off.
    // Exercises the subject-line render, market date placement, and the
    // locale-aware salutation/sign-off detection (previously English/German-only).
    let text = "Max Mustermann\n\nBetreff: Bewerbung als Frontend Engineer\n\nSehr geehrte Damen und Herren,\n\nmit großem Interesse bewerbe ich mich.\n\nMit freundlichen Grüßen\nMax Mustermann";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: Some("de".to_string()),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("German cover letter renders");
    assert!(!bytes.is_empty());
}

#[test]
fn test_generate_pdf_cover_letter_french_salutation() {
    // French salutation/sign-off must be recognized (not dumped into the
    // recipient block) now that detection is locale-aware.
    let text = "Marie Dupont\n\nMadame, Monsieur,\n\nje vous écris pour le poste.\n\nCordialement,\nMarie Dupont";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: Some("fr".to_string()),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("French cover letter renders");
    assert!(!bytes.is_empty());
}

/// Every market id `resolveMarket` can return (values of `COUNTRY_TO_MARKET` /
/// `LANGUAGE_TO_MARKET` plus the `intl` fallback in
/// `packages/prompts/src/locale/market.ts`) must export. Regression: `intl`
/// leaked into Typst's `#set text(lang: ..)` via `target_lang()` and failed the
/// render (#1552). Keep in sync with that TS source.
#[test]
fn every_resolve_market_id_exports_a_resume_and_a_cover_letter() {
    const MARKETS: &[&str] = &[
        "intl", "us", "uk", "de", "at", "ch", "fr", "es", "it", "pt", "br", "tr", "ru", "cn", "jp",
        "kr",
    ];
    for market in MARKETS {
        for document_type in [DocumentType::Resume, DocumentType::CoverLetter] {
            let request = ExportRequest {
                text: "Jane Doe
jane@example.com

EXPERIENCE
Acme  2020 - Present
Did things"
                    .to_string(),
                format: ExportFormat::Pdf,
                document_type,
                template_id: TemplateId::SwissMinimal,
                meta: None,
                ats_mode: false,
                locale: Some((*market).to_string()),
                contact: None,
                accent: None,
                letter_layout: LetterLayout::Classic,
            };
            assert!(
                generate_pdf(&request).is_ok(),
                "market `{market}` ({document_type:?}) must export"
            );
        }
    }
}

/// A market id is not a language: `uk` must not become Ukrainian, `br` Breton.
#[test]
fn target_lang_maps_market_ids_to_their_language() {
    let lang = |locale: &str| {
        ExportRequest {
            text: String::new(),
            format: ExportFormat::Pdf,
            document_type: DocumentType::Resume,
            template_id: TemplateId::SwissMinimal,
            meta: None,
            ats_mode: false,
            locale: Some(locale.to_string()),
            contact: None,
            accent: None,
            letter_layout: LetterLayout::Classic,
        }
        .target_lang()
    };
    for (market, expected) in [
        ("intl", "en"),
        ("uk", "en"),
        ("us", "en"),
        ("ch", "de"),
        ("br", "pt"),
        ("jp", "ja"),
        ("de", "de"),
        ("fr", "fr"),
        ("de-DE", "de"),
    ] {
        assert_eq!(lang(market), expected, "market `{market}`");
    }
}
