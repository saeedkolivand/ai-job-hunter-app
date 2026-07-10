//! Tests for the Typst engine — smoke tests + model-based + ATS harness.
//!
//! All tests run fully in-process (no disk, no network) via the offline
//! ResumeWorld hard-wall.

use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{
    render_letter_pdf, render_letter_svg_pages, render_pdf, render_pdf_from_source,
    render_resume_svg_pages, RenderOpts, TypstTemplate,
};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Count `/Type /Page` (individual page) objects in PDF bytes.
///
/// Uses a byte-level scan rather than lopdf's `get_pages()` because lopdf's
/// page-tree walker does not handle all page-tree structures that Typst emits
/// (it misses pages under certain indirect-reference trees and returns 1 even
/// for multi-page documents). The scan finds all occurrences of the `/Type`
/// `/Page` dictionary entry that marks an individual page object (not `/Type`
/// `/Pages` which marks a page-tree node).
///
/// Tolerates zero-or-more spaces between `/Type` and `/Page`: typst-pdf 0.15's
/// krilla/pdf-writer backend serialises dict entries as `/Type/Page` (no
/// space), where the pinned 0.14.2 backend wrote `/Type /Page` (one space).
/// Matching both keeps this scan from silently reporting zero pages again on
/// the next writer-formatting tweak.
fn count_pdf_pages(bytes: &[u8]) -> usize {
    let key = b"/Type";
    let val = b"/Page";
    let mut count = 0usize;
    let mut i = 0usize;
    while i + key.len() < bytes.len() {
        if bytes[i..i + key.len()] == *key {
            let mut j = i + key.len();
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if bytes[j..].starts_with(val) {
                // The character after `/Page` must not be `s` (which would make it `/Pages`).
                if bytes.get(j + val.len()) != Some(&b's') {
                    count += 1;
                }
            }
            i += key.len();
        } else {
            i += 1;
        }
    }
    count
}

/// Extract every `/Link` annotation target URI from a rendered PDF.
///
/// Typst writes `/Annots` as an array of **inline dictionaries**; lopdf's
/// `get_page_annotations` only resolves *indirect references* and so misses them
/// entirely — the documented regression that once made every header link read as
/// "missing". We therefore walk each object's `/Annots` array ourselves (mirroring
/// the validator's reader) and pull `/A /URI` off each `/Link`.
fn link_uris(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered PDF should parse with lopdf");

    fn uri_of(annot: &lopdf::Dictionary, doc: &lopdf::Document) -> Option<String> {
        let is_link = annot
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if !is_link {
            return None;
        }
        annot
            .get(b"A")
            .ok()
            .and_then(|a| match a {
                lopdf::Object::Dictionary(d) => Some(d.clone()),
                lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
                _ => None,
            })
            .and_then(|d| {
                d.get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            })
    }

    let mut uris = Vec::new();
    for obj in doc.objects.values() {
        let Ok(dict) = obj.as_dict() else {
            continue;
        };
        let array = match dict.get(b"Annots") {
            Ok(lopdf::Object::Array(a)) => a.clone(),
            Ok(lopdf::Object::Reference(id)) => {
                match doc.get_object(*id).and_then(|o| o.as_array()) {
                    Ok(a) => a.clone(),
                    Err(_) => continue,
                }
            }
            _ => continue,
        };
        for entry in &array {
            let annot = match entry {
                lopdf::Object::Dictionary(d) => d.clone(),
                lopdf::Object::Reference(id) => match doc.get_dictionary(*id) {
                    Ok(d) => d.clone(),
                    Err(_) => continue,
                },
                _ => continue,
            };
            if let Some(uri) = uri_of(&annot, &doc) {
                uris.push(uri);
            }
        }
    }
    uris
}

// ── Smoke tests (raw source path) ─────────────────────────────────────────────

/// Minimal Typst document that exercises font loading and basic layout.
const SMOKE_SOURCE: &str = "= Hello\n\nSome body text rendered with the bundled font.";

#[test]
fn smoke_pdf_is_non_empty_and_starts_with_pdf_header() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    assert!(!bytes.is_empty(), "rendered PDF must be non-empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "rendered PDF must begin with %PDF, got: {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn smoke_pdf_text_extraction_contains_expected_words() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract should be able to read our output");

    let lower = extracted.to_lowercase();
    assert!(
        lower.contains("hello"),
        "extracted text should contain 'hello'; got: {extracted:?}"
    );
    assert!(
        lower.contains("body"),
        "extracted text should contain 'body'; got: {extracted:?}"
    );
}

// ── Fixture ───────────────────────────────────────────────────────────────────

/// Short one-page resume fixture — enough content to exercise all block types
/// (header, paragraph, entry with bullets, standalone bullets) while keeping
/// compilation fast.
const FIXTURE_RESUME: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe

SUMMARY
Experienced software engineer with a passion for building reliable systems.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Designed distributed task scheduler reducing latency by 40 percent
- Led migration to Rust-based microservices across three product teams

Software Engineer | Beta Inc | 2018 – 2021
- Built real-time data pipeline processing one million events per day
- Mentored two junior engineers through onboarding

EDUCATION
B.Sc. Computer Science | State University | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

// ── Model-based render tests ──────────────────────────────────────────────────

fn opts_a4() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

#[test]
fn classic_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");

    assert!(!bytes.is_empty(), "PDF bytes must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "output must start with %PDF header"
    );
}

// ── SVG live-preview emit ───────────────────────────────────────────────────────
//
// The live preview renders the SAME model + SAME Typst world as the PDF export,
// emitting one SVG string per page instead of a PDF blob. These guard that the
// SVG sibling fns return ≥1 non-empty page whose string is a real SVG document.

#[test]
fn render_resume_svg_pages_returns_svg_page() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let pages = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_resume_svg_pages(classic) should succeed");

    assert!(
        !pages.is_empty(),
        "résumé preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "résumé preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

#[test]
fn document_accent_overrides_letter_accent_color() {
    use super::letter::style_from_template as letter_style_from_template;

    // Cover letters inherit the résumé template's accent. A document accent
    // applied via `Template::with_accent_override` must surface as the letter's
    // `c_accent`; a malformed value must leave the template's palette intact.
    let base_accent = letter_style_from_template(&Template::get(TemplateId::Classic)).c_accent;

    let overridden = Template::get(TemplateId::Classic).with_accent_override(Some("#AA0000"));
    assert_eq!(
        letter_style_from_template(&overridden).c_accent,
        "#AA0000",
        "a valid document accent must recolor the letter accent"
    );

    let malformed = Template::get(TemplateId::Classic).with_accent_override(Some("nope"));
    assert_eq!(
        letter_style_from_template(&malformed).c_accent,
        base_accent,
        "a malformed accent must leave the letter palette unchanged"
    );
}

#[test]
fn render_letter_svg_pages_returns_svg_page() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages =
        render_letter_svg_pages(LETTER_FIXTURE_US, &t, None, Some("Jane Smith"), "us", "en")
            .expect("render_letter_svg_pages(us) should succeed");

    assert!(
        !pages.is_empty(),
        "cover-letter preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "cover-letter preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

// ── Link-annotation round-trip (header contact links) ───────────────────────────
//
// The header carries the candidate's email/LinkedIn/GitHub as clickable links.
// They must survive into the PDF as real `/Link` annotations with extractable
// `/A /URI` targets — the exact path that regressed before (lopdf inline-annot
// parsing). Render through the live engine, then read the links back. Replaces the
// `resume_embeds_contact_link_annotations` + `every_template_renders_a_valid_pdf`
// coverage that lived in the deleted printpdf `layout_pdf` suite.

#[test]
fn classic_resume_embeds_contact_link_annotations() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from classic resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from classic resume; found {uris:?}"
    );
}

#[test]
fn two_column_resume_embeds_contact_link_annotations() {
    // The full-width header in two-column templates is the higher-risk path for
    // dropped annotations, so assert links survive there too (Atelier).
    let model = model_from_resume_text(FIXTURE_RESUME);
    let template = Template::get(TemplateId::Atelier);
    let bytes = render_pdf(
        &model,
        TypstTemplate::from_template(&template),
        &opts_a4(),
        Some(&template),
    )
    .expect("render_pdf(atelier) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from two-column resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from two-column resume; found {uris:?}"
    );
}

#[test]
fn every_template_renders_a_valid_pdf() {
    // Canonical user-facing set — must match the `TemplateId` enum (pinned by the
    // serde round-trip test in types.rs and the TS sync guard).
    let ids = [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Atelier,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Portrait,
        TemplateId::Lebenslauf,
        TemplateId::Cadence,
        TemplateId::Regent,
        TemplateId::Aria,
        TemplateId::Saffron,
    ];
    assert_eq!(ids.len(), 12, "expected the twelve canonical templates");

    let model = model_from_resume_text(FIXTURE_RESUME);
    for id in ids {
        let template = Template::get(id);
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts_a4(),
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("render_pdf({id:?}) should succeed: {e:?}"));
        assert!(!bytes.is_empty(), "{id:?}: PDF bytes must not be empty");
        assert!(
            bytes.starts_with(b"%PDF"),
            "{id:?}: output must start with %PDF"
        );
        assert!(
            count_pdf_pages(&bytes) >= 1,
            "{id:?}: must emit at least one page"
        );
    }
}

#[test]
fn classic_render_letter_page_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 215.9,
            height_mm: 279.4,
        },
        lang: "en".to_string(),
        accent: None,
        ats: false,
    };
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf(classic, Letter) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_valid_accent_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#1a2b3c".to_string()),
        lang: "en".to_string(),
        ats: false,
    };
    // Classic now renders through the parametric SingleColumn template, which
    // honors the accent override (data.opts.accent) — it must not crash.
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with a valid accent override");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_invalid_accent_falls_back_gracefully() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("not-a-color".to_string()),
        lang: "en".to_string(),
        ats: false,
    };
    // Invalid accent must not cause an error (normalise_accent returns "" →
    // template defaults apply).
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with an invalid accent color");
    assert!(bytes.starts_with(b"%PDF"));
}

// ── ATS harness ───────────────────────────────────────────────────────────────
//
// Renders the fixture through Classic, extracts text with pdf-extract, and
// asserts three ATS-safety properties:
//
//   (a) READING ORDER — section headings appear in the expected top-to-bottom
//       order (SUMMARY before EXPERIENCE before EDUCATION before SKILLS).
//
//   (b) WORD BOUNDARIES — a known multi-word phrase from the fixture survives
//       WITH spaces and is not run together (e.g. "State University" not
//       "StateUniversity").
//
//   (c) CONTENT PRESENT — the candidate name, all major section headings, and
//       a representative bullet fragment are findable in the extracted text.

#[test]
fn ats_harness_classic_reading_order_word_boundaries_content() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) for ATS harness");

    let extracted =
        pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed on our output");

    let lower = extracted.to_lowercase();

    // ── (c) Content present ───────────────────────────────────────────────────
    assert!(
        lower.contains("jane doe"),
        "ATS: candidate name 'Jane Doe' missing from extracted text\n---\n{extracted}"
    );

    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "ATS: section heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // A bullet fragment that must survive intact.
    assert!(
        lower.contains("distributed task scheduler"),
        "ATS: bullet fragment 'distributed task scheduler' missing\n---\n{extracted}"
    );

    // ── (b) Word boundaries ───────────────────────────────────────────────────
    // "State University" must appear with a space, not run together.
    assert!(
        lower.contains("state university"),
        "ATS: 'state university' must appear with preserved word boundary\n---\n{extracted}"
    );

    // ── (a) Reading order ─────────────────────────────────────────────────────
    let order = ["summary", "experience", "education", "skills"];
    let mut last_pos = 0usize;
    for heading in &order {
        let pos = lower.find(heading).unwrap_or_else(|| {
            panic!("ATS reading order: '{heading}' not found in extracted text")
        });
        assert!(
            pos >= last_pos,
            "ATS reading order: '{heading}' (at {pos}) appeared before previous heading (at {last_pos})\n---\n{extracted}"
        );
        last_pos = pos;
    }
}

// ── Accent normalisation unit tests (in render.rs tests, but also here) ───────

#[test]
fn render_opts_default_is_a4_en() {
    let opts = RenderOpts::default();
    assert_eq!(opts.page.width_mm, 210.0);
    assert_eq!(opts.page.height_mm, 297.0);
    assert_eq!(opts.lang, "en");
    assert!(!opts.ats);
    assert!(opts.accent.is_none());
}

// ── JsonSection.kind + emphasize_education unit tests ─────────────────────────
//
// These tests verify the two new data-model fields without needing a PDF render:
//
//   (a) section_id_to_kind: education section serializes as kind == "education".
//   (b) style_from_template: academic → emphasize_education == true;
//       swiss_minimal → emphasize_education == false.

#[test]
fn json_section_kind_education_serializes_correctly() {
    use crate::export::typst_engine::render::{section_id_to_kind, JsonSection};
    use crate::model::document::SectionId;

    let kind = section_id_to_kind(&SectionId::Education);
    assert_eq!(
        kind, "education",
        "SectionId::Education must serialize to \"education\""
    );

    // Spot-check a few other kinds while we are here.
    assert_eq!(section_id_to_kind(&SectionId::Experience), "experience");
    assert_eq!(section_id_to_kind(&SectionId::Skills), "skills");
    assert_eq!(
        section_id_to_kind(&SectionId::Custom("Foo".into())),
        "custom"
    );

    // Confirm a JsonSection round-trips through serde with kind present.
    let section = JsonSection {
        heading: "Education".to_string(),
        blocks: vec![],
        placement: "main".to_string(),
        kind: kind.clone(),
    };
    let json = serde_json::to_string(&section).expect("JsonSection must serialize");
    assert!(
        json.contains("\"kind\":\"education\""),
        "serialized JSON must contain \"kind\":\"education\"; got: {json}"
    );
}

#[test]
fn style_from_template_emphasize_education_academic_true_others_false() {
    use crate::export::typst_engine::render::style_from_template;

    let academic = template_style(TemplateId::Academic);
    assert!(
        style_from_template(&academic).emphasize_education,
        "Academic template must have emphasize_education == true"
    );

    let swiss = template_style(TemplateId::SwissMinimal);
    assert!(
        !style_from_template(&swiss).emphasize_education,
        "SwissMinimal template must have emphasize_education == false"
    );
}

// ── Atelier (Phase 1b) tests ──────────────────────────────────────────────────
//
// Tests cover:
//   (1) Basic render — valid PDF in both ats:false and ats:true.
//   (2) 2-page sidebar repeat — enough content to force ≥2 pages; ALL sidebar
//       items from the fixture must be present in the extracted text (regression
//       guard for the dense-sidebar overflow fix, F1/F4).
//   (3) ATS collapse — ats:true → linear reading order, sidebar headings appear
//       AFTER the main-column headings but still present and in order.
//   (4) Entry integrity — titles + bullets present in extracted text.
//   (5) Accent override — custom accent does not cause a compile error.
//   (6) Sample PDF written to target/ for human review (informational, always passes).
//   (7) Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certs, 4 languages;
//       every sidebar item must be present (F1 regression guard).
//   (8) Empty-sidebar fixture — all sections placed in main; no sidebar sections;
//       template must fall back to single-column (no band) and render cleanly.

/// Single-page fixture — exercises all block types.
const ATELIER_FIXTURE: &str = "\
Alexandra Rivera
alex@example.com | [LinkedIn](https://linkedin.com/in/alexrivera) | https://alexrivera.dev

SUMMARY
Product-focused engineering leader with twelve years building distributed systems.

EXPERIENCE
Principal Engineer | Meridian Systems | 2019 – Present
- Scaled the event-sourcing platform to 500 k events per second
- Drove adoption of a domain-driven architecture across seven product teams

Software Engineer | Cobalt Labs | 2015 – 2019
- Built the real-time collaboration layer used by 200 k active users
- Reduced cold-start latency from 900 ms to 110 ms

EDUCATION
M.Sc. Computer Science | Western University | 2013 – 2015

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, Kafka, PostgreSQL

LANGUAGES
English (native), Portuguese (fluent)
";

/// Multi-page fixture — enough experience + project entries to force ≥2 pages.
/// The main-column content (SUMMARY + EXPERIENCE + PROJECTS) is deliberately
/// long enough to overflow a single A4 page in the 70% main column.
const ATELIER_MULTIPAGE: &str = "\
Alexandra Rivera
alex@example.com | https://alexrivera.dev

SUMMARY
Engineering leader with a decade of distributed-systems experience building resilient
platforms at scale. Passionate about developer productivity, reliability engineering,
and growing high-performing teams across multiple time zones.

EXPERIENCE
Staff Engineer | Apex Corp | 2022 – Present
- Led the platform-reliability initiative that reduced P99 latency by 60 percent across all production services
- Introduced chaos engineering practices that were adopted across twelve service teams globally
- Architected a zero-downtime schema migration pipeline managing a 10 TB customer dataset
- Mentored eight engineers through promotion to senior level over the course of eighteen months
- Drove the company-wide observability strategy resulting in 99.99 percent annual SLA achievement
- Defined engineering excellence standards that were subsequently adopted by all thirty backend teams
- Designed the on-call runbook system reducing mean time to resolution from 45 minutes to 8 minutes

Senior Engineer | Meridian Systems | 2019 – 2022
- Built the multi-tenant billing engine that processed 50 M transactions per month without downtime
- Migrated a legacy monolith to fifty domain-aligned microservices over an eighteen-month programme
- Designed the event-sourcing backbone now serving 300 k events per second at peak production load
- Reduced infrastructure cost by 35 percent through adaptive auto-scaling policies and spot instances
- Shipped a real-time analytics dashboard that was adopted by over 10 k business users on launch day
- Onboarded and technically led a distributed team of nine engineers across three time zones

Software Engineer | Cobalt Labs | 2016 – 2019
- Delivered the real-time collaboration layer for the flagship product used by 200 k daily active users
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Reduced cold-start API latency from 900 ms to 110 ms through optimised connection pooling strategies
- Contributed core modules to three open-source libraries with a combined 8 k GitHub stars

Junior Software Engineer | Vertex Startup | 2014 – 2016
- Shipped the initial iOS client that reached 50 k downloads in the first month after public launch
- Rebuilt the search indexing pipeline and cut ingestion lag from five minutes to eight seconds
- Integrated third-party payment providers handling 500 k transactions per day in a PCI-DSS environment

PROJECTS
Distributed Rate Limiter | Open Source | 2021
- Designed a Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published to crates.io; adopted by fourteen organisations within six months of initial release
- Maintained comprehensive documentation, changelog, and semver-stable public API

High-Throughput Log Aggregator | Open Source | 2020
- Built a lock-free ring-buffer pipeline aggregating 1 M log lines per second on commodity hardware
- Presented at a regional systems-programming conference to an audience of 400 engineers

EDUCATION
M.Sc. Computer Science | Western University | 2012 – 2014
B.Sc. Computer Engineering | Eastern College | 2008 – 2012

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform, Prometheus, Grafana

LANGUAGES
English (native), Portuguese (fluent), Spanish (working)
";

/// Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certifications, 4 languages.
/// This is the F1 regression fixture: the sidebar content is tall enough that
/// the template must detect overflow and fall back to single-column so that
/// no sidebar item is silently clipped.
const ATELIER_DENSE_SIDEBAR: &str = "\
Jordan Kim
jordan@example.com | https://linkedin.com/in/jordankim | https://jordankim.dev

SUMMARY
Polyglot engineer with deep expertise in distributed systems and cloud infrastructure.

EXPERIENCE
Senior Platform Engineer | Globex Corp | 2020 – Present
- Designed a multi-region failover system achieving five nines availability
- Reduced mean deployment time from 45 minutes to under four minutes

Platform Engineer | Initech Solutions | 2017 – 2020
- Built a shared CI/CD platform adopted by 80 engineering teams
- Introduced contract testing reducing integration failures by 70 percent

EDUCATION
M.Eng. Software Engineering | Metro University | 2015 – 2017
B.Sc. Computer Science | Coastal College | 2011 – 2015

SKILLS
Rust, Go, Python, TypeScript, Java, Kotlin, C++, Bash, SQL, Terraform, Ansible, Pulumi

LANGUAGES
English (native), German (fluent), French (professional), Mandarin (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Google Cloud Professional Data Engineer
Certified Kubernetes Administrator
";

fn opts_atelier(ats: bool) -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#4A4580".to_string()),
        lang: "en".to_string(),
        ats,
    }
}

// (1a) Non-ATS render produces a valid PDF.
#[test]
fn atelier_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    assert!(!bytes.is_empty(), "PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"), "output must start with %PDF");
}

// (1b) ATS render also produces a valid PDF.
#[test]
fn atelier_ats_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    assert!(!bytes.is_empty(), "ATS PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "ATS output must start with %PDF"
    );
}

// (2) 2-page sidebar repeat: the multi-page fixture forces ≥2 pages.
// The FULL set of sidebar items from the multipage fixture must be present
// in the extracted text — this is the regression guard for F1/F4 (dense
// sidebar overflow).  A clipped sidebar would cause these assertions to fail.
#[test]
fn atelier_multipage_sidebar_renders_once() {
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    // Assert ≥2 pages by counting /Type /Page objects directly in the PDF bytes.
    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on our Typst PDF");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a single
    // space so line-wrapped tokens ("Eastern \nCollege") still match. Education
    // entries are now rendered as entry blocks (grid layout) which can introduce
    // line breaks inside multi-word names — normalization makes assertions robust.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Every sidebar skill from the fixture must be present.
    let sidebar_skills = [
        "rust",
        "go",
        "typescript",
        "kubernetes",
        "aws",
        "gcp",
        "kafka",
        "postgresql",
        "redis",
        "terraform",
        "prometheus",
        "grafana",
    ];
    for skill in &sidebar_skills {
        assert!(
            lower.contains(skill),
            "sidebar skill '{skill}' missing from extracted text — possible sidebar clip\n---\n{lower}"
        );
    }

    // Education entries in the sidebar must also be present.
    assert!(
        lower.contains("western university"),
        "sidebar education 'western university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("eastern college"),
        "sidebar education 'eastern college' missing\n---\n{lower}"
    );

    // Languages must be present.
    for lang in &["english", "portuguese", "spanish"] {
        assert!(
            lower.contains(lang),
            "sidebar language '{lang}' missing\n---\n{lower}"
        );
    }

    // The sidebar now renders ONCE (page 1 only), no longer repeated per page.
    // A sidebar-only skill ("Grafana" — never appears in a main-column bullet)
    // must therefore appear exactly once across the whole multi-page document.
    let grafana_count = lower.matches("grafana").count();
    assert_eq!(
        grafana_count, 1,
        "sidebar skill 'Grafana' must appear exactly once (sidebar renders once, \
         not repeated per page); found {grafana_count}\n---\n{lower}"
    );
}

#[test]
fn portrait_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;

    // Same multi-page fixture through Portrait (no photo). Portrait uses the same
    // page(background:) sidebar technique, so the page-1-only gate must hold here too.
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));

    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower = extracted.to_lowercase();
    // Sidebar content present (on page 1) …
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    // … and rendered exactly once, not repeated per page.
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Portrait sidebar must render once across pages\n---\n{lower}"
    );
}

// (3) ATS collapse: ats:true → single column, linear reading order.
// Main headings (SUMMARY, EXPERIENCE) must appear before sidebar headings
// (EDUCATION, SKILLS, LANGUAGES) in the extracted text.
#[test]
fn atelier_ats_linear_reading_order() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // All major section headings must be present.
    for heading in &["summary", "experience", "education", "skills", "languages"] {
        assert!(
            lower.contains(heading),
            "ATS: heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // Main-column sections must appear before sidebar-column sections in the
    // extracted text (linear order = no column interleaving).
    let pos_experience = lower
        .find("experience")
        .expect("'experience' must be present");
    let pos_education = lower
        .find("education")
        .expect("'education' must be present");
    let pos_skills = lower.find("skills").expect("'skills' must be present");

    assert!(
        pos_experience < pos_education,
        "ATS: 'experience' ({pos_experience}) should precede 'education' ({pos_education}) \
         in linear order\n---\n{extracted}"
    );
    assert!(
        pos_experience < pos_skills,
        "ATS: 'experience' ({pos_experience}) should precede 'skills' ({pos_skills}) \
         in linear order\n---\n{extracted}"
    );

    // Word boundaries: "Western University" must appear with a space.
    assert!(
        lower.contains("western university"),
        "ATS: 'western university' must appear with preserved word boundary\n---\n{extracted}"
    );
}

// (4) Entry integrity: entry titles and bullet fragments must all be present.
#[test]
fn atelier_entry_integrity() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // Candidate name.
    assert!(
        lower.contains("alexandra rivera"),
        "entry integrity: candidate name missing\n---\n{extracted}"
    );

    // Entry titles.
    for title in &["meridian systems", "cobalt labs"] {
        assert!(
            lower.contains(title),
            "entry integrity: title '{title}' missing\n---\n{extracted}"
        );
    }

    // Bullet fragments.
    assert!(
        lower.contains("event-sourcing platform"),
        "entry integrity: bullet fragment 'event-sourcing platform' missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("real-time collaboration"),
        "entry integrity: bullet fragment 'real-time collaboration' missing\n---\n{extracted}"
    );
}

// (5) Custom accent override does not cause a compile error.
#[test]
fn atelier_custom_accent_succeeds() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#1A6B5A".to_string()), // deep teal override
        lang: "en".to_string(),
        ats: false,
    };
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts, None)
        .expect("render_pdf(atelier, custom accent) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

// (6) Write a classic sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn classic_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("classic_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("classic_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Classic sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "classic_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (6b) Write an atelier sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn atelier_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (6c) Write a MULTI-PAGE atelier sample to target/ for human review.
// Forces ≥2 pages so the page-background sidebar repeat + pagination + the
// locked house spacing scale can be eyeballed across a page break.
// Informational; .ok()-style write never fails the run.
#[test]
fn atelier_write_multipage_sample_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_multipage_sample_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_multipage_diag.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier multipage sample written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_multipage_sample_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (7) Dense-sidebar fixture: 10+ skills, 2 degrees, 3 certs, 4 languages.
// Every sidebar item must appear in the extracted PDF text, proving that
// the dense-sidebar overflow detection (F1) correctly falls back to
// single-column and does NOT silently clip any content.
#[test]
fn atelier_dense_sidebar_no_data_loss() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_DENSE_SIDEBAR);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, dense-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "dense-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on dense-sidebar output");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a
    // single space so that line-wrapped tokens ("Coastal \nCollege") still
    // match the expected substrings.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // ── Skills (10+) ──────────────────────────────────────────────────────────
    let skills = [
        "rust",
        "go",
        "python",
        "typescript",
        "java",
        "kotlin",
        "c++",
        "bash",
        "sql",
        "terraform",
        "ansible",
        "pulumi",
    ];
    for skill in &skills {
        assert!(
            lower.contains(skill),
            "dense-sidebar: skill '{skill}' missing — possible silent clip\n---\n{lower}"
        );
    }

    // ── Education (2 degrees) ─────────────────────────────────────────────────
    assert!(
        lower.contains("metro university"),
        "dense-sidebar: 'metro university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("coastal college"),
        "dense-sidebar: 'coastal college' missing\n---\n{lower}"
    );

    // ── Languages (4) ─────────────────────────────────────────────────────────
    for lang in &["english", "german", "french", "mandarin"] {
        assert!(
            lower.contains(lang),
            "dense-sidebar: language '{lang}' missing\n---\n{lower}"
        );
    }

    // ── Certifications (3) ────────────────────────────────────────────────────
    assert!(
        lower.contains("aws solutions architect"),
        "dense-sidebar: 'aws solutions architect' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("google cloud"),
        "dense-sidebar: 'google cloud' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("kubernetes administrator"),
        "dense-sidebar: 'kubernetes administrator' cert missing\n---\n{lower}"
    );

    // Write the dense-sidebar sample for eyeballing.
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_dense_sidebar_no_data_loss: could not create target/: {e}");
    }
    let out_path = target.join("atelier_dense_sidebar.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!(
            "Dense-sidebar sample PDF written to: {}",
            out_path.display()
        ),
        Err(e) => eprintln!(
            "atelier_dense_sidebar_no_data_loss: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
}

// (8) Empty-sidebar fixture: all sections map to main; no sidebar sections.
// The template must render cleanly in single-column mode (no band) and all
// content must be present in the extracted text.
#[test]
fn atelier_empty_sidebar_renders_single_column() {
    // A resume with only SUMMARY + EXPERIENCE + PROJECTS — none of these
    // sections map to the sidebar (Skills/Education/Languages/Certifications
    // are the sidebar sections).  The template must detect no sidebar sections
    // and fall back to single-column to avoid rendering an empty tinted band.
    let fixture = "\
Morgan Ellis
morgan@example.com | https://morganellis.dev

SUMMARY
Full-stack engineer specialising in high-throughput data pipelines.

EXPERIENCE
Senior Engineer | DataCo | 2020 – Present
- Designed a streaming ingestion layer processing 2 M events per second
- Reduced P99 query latency from 800 ms to 35 ms via index optimisation

Engineer | PipeCraft | 2017 – 2020
- Built the core ETL framework adopted by all twelve data teams
- Migrated a batch pipeline to a streaming architecture with zero downtime

PROJECTS
OpenStream | Open Source | 2022
- High-throughput event router with pluggable backends
- 2 k GitHub stars; used in production by three Fortune 500 companies
";

    let model = model_from_resume_text(fixture);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, empty-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "empty-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on empty-sidebar output");

    let lower = extracted.to_lowercase();

    // All content must be present — none clipped by a missing sidebar.
    assert!(
        lower.contains("morgan ellis"),
        "empty-sidebar: candidate name missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("dataco"),
        "empty-sidebar: 'dataco' entry missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("streaming ingestion"),
        "empty-sidebar: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("openstream"),
        "empty-sidebar: project 'openstream' missing\n---\n{extracted}"
    );
}

// ── Phase 2: Classic, SwissMinimal, Academic — SingleColumn parametric ────────
//
// For each new template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).

fn template_style(id: TemplateId) -> Template {
    Template::get(id)
}

fn opts_sc() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

// ── Swiss Minimal ─────────────────────────────────────────────────────────────

#[test]
fn swiss_minimal_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) should succeed");
    assert!(!bytes.is_empty(), "Swiss Minimal PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Swiss Minimal output must start with %PDF"
    );
}

#[test]
fn swiss_minimal_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on swiss-minimal output");
    let lower = extracted.to_lowercase();

    assert!(
        lower.contains("jane doe"),
        "swiss-minimal ATS: 'jane doe' missing\n---\n{extracted}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "swiss-minimal ATS: heading '{heading}' missing\n---\n{extracted}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "swiss-minimal ATS: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("state university"),
        "swiss-minimal ATS: 'state university' word boundary broken\n---\n{extracted}"
    );

    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("swiss-minimal ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "swiss-minimal ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

#[test]
fn swiss_minimal_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("swiss_minimal_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("swiss_minimal_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Swiss Minimal sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "swiss_minimal_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Academic ──────────────────────────────────────────────────────────────────

#[test]
fn academic_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) should succeed");
    assert!(!bytes.is_empty(), "Academic PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Academic output must start with %PDF"
    );
}

#[test]
fn academic_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on academic output");
    let lower = extracted.to_lowercase();

    assert!(
        lower.contains("jane doe"),
        "academic ATS: 'jane doe' missing\n---\n{extracted}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "academic ATS: heading '{heading}' missing\n---\n{extracted}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "academic ATS: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("state university"),
        "academic ATS: 'state university' word boundary broken\n---\n{extracted}"
    );

    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("academic ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "academic ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

#[test]
fn academic_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("academic_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("academic_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Academic sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "academic_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Phase 1c: Cover-letter render tests ───────────────────────────────────────
//
// Tests cover:
//   (1) US letter — renders valid PDF on Letter-size page (215.9 × 279.4 mm),
//       no subject line; salutation, body phrase, sign-off present.
//   (2) DE letter — renders valid PDF on A4; DIN subject "Betreff:" present;
//       German salutation + signoff recognised.
//   (3) Both PDFs start with %PDF.
//   (4) Sample PDF writers: target/letter_us_sample.pdf and
//       target/letter_de_sample.pdf — informational, always pass.

/// US English cover letter fixture.
const LETTER_FIXTURE_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. With five years of experience building distributed systems in Rust and \
Go, I believe I would be a great addition to your team.

During my time at Beta Inc, I led the migration of our payments service to a \
microservices architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";

/// German DIN 5008 cover letter fixture.
const LETTER_FIXTURE_DE: &str = "\
Max Müller
max@example.de | https://linkedin.com/in/maxmueller

Frankfurt, 2. Juni 2025

Frau Dr. Anna Weber
Musterfirma GmbH
Hauptstraße 1
60311 Frankfurt am Main

Betreff: Bewerbung als Software Engineer

Sehr geehrte Frau Dr. Weber,

mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als \
Software Engineer gelesen. Ich bewerbe mich hiermit für diese Stelle.

In meiner bisherigen Tätigkeit bei der Beta GmbH habe ich umfangreiche Erfahrungen \
in der Entwicklung verteilter Systeme gesammelt und konnte die Systemlatenz um \
40 Prozent reduzieren.

Über eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen.

Mit freundlichen Grüßen,

Max Müller
";

// (1) US letter renders to a valid PDF and contains expected text.
#[test]
fn letter_us_renders_valid_pdf_with_expected_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(LETTER_FIXTURE_US, &t, None, Some("Jane Smith"), "us", "en")
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
}

// (2) DE letter renders to a valid PDF and contains DIN subject + German conventions.
#[test]
fn letter_de_renders_valid_pdf_with_subject_line() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(LETTER_FIXTURE_DE, &t, None, Some("Max Müller"), "de", "de")
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
}

// (3) Both outputs start with %PDF — belt-and-suspenders after the content tests
// above already assert this; kept as a quick standalone guard.
#[test]
fn letter_us_and_de_both_start_with_pdf_header() {
    let t = Template::get(TemplateId::SwissMinimal);
    let us = render_letter_pdf(LETTER_FIXTURE_US, &t, None, Some("Jane Smith"), "us", "en")
        .expect("render_letter_pdf(us)");
    let de = render_letter_pdf(LETTER_FIXTURE_DE, &t, None, Some("Max Müller"), "de", "de")
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
    let bytes = render_letter_pdf(LETTER_FIXTURE_US, &t, None, Some("Jane Smith"), "us", "en")
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
    let bytes = render_letter_pdf(LETTER_FIXTURE_DE, &t, None, Some("Max Müller"), "de", "de")
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

// ── Phase 3a: Meridian, Throughline, Quanta — premium single-column ───────────
//
// For each template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).
//
// For Throughline additionally:
//   (d) EXPERIENCE entries + bullets all survive extraction (timeline decoration
//       must not drop any text).

fn opts_p3a() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

// ── Meridian ──────────────────────────────────────────────────────────────────

#[test]
fn meridian_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed");
    assert!(!bytes.is_empty(), "Meridian PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Meridian output must start with %PDF"
    );
}

#[test]
fn meridian_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on meridian output");

    // Normalise whitespace — band layout can introduce line breaks inside
    // the header content (name, contact line placed in page background).
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "meridian ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "meridian ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "meridian ATS: bullet fragment 'distributed task scheduler' missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "meridian ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order: summary → experience → education → skills.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("meridian ATS: '{h}' not found in extracted text"));
        assert!(
            pos >= last,
            "meridian ATS: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

#[test]
fn meridian_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("meridian_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("meridian_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Meridian sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "meridian_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Throughline ───────────────────────────────────────────────────────────────

#[test]
fn throughline_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed");
    assert!(!bytes.is_empty(), "Throughline PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Throughline output must start with %PDF"
    );
}

#[test]
fn throughline_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on throughline output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "throughline ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "throughline ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline ATS: bullet fragment missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "throughline ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("throughline ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "throughline ATS: '{h}' ({pos}) appeared before previous ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

// (d) Throughline-specific: EXPERIENCE entries + bullets must all survive
// text extraction — the timeline decoration (nodes/spine) must not drop content.
#[test]
fn throughline_timeline_entries_and_bullets_survive_extraction() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for timeline integrity");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Entry titles from the fixture's EXPERIENCE section.
    for title in &["acme corp", "beta inc"] {
        assert!(
            lower.contains(title),
            "throughline timeline: entry title '{title}' missing — timeline may have dropped text\n---\n{lower}"
        );
    }

    // Bullet fragments from EXPERIENCE entries.
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline timeline: bullet 'distributed task scheduler' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("real-time data pipeline"),
        "throughline timeline: bullet 'real-time data pipeline' missing\n---\n{lower}"
    );
}

#[test]
fn throughline_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("throughline_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("throughline_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Throughline sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "throughline_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── (Quanta removed) ──────────────────────────────────────────────────────────

// ── Phase 3b-i: Portrait + Lebenslauf — photo templates ──────────────────────
//
// Tests cover, per template:
//   (1) Render with fixture photo → valid PDF.
//   (2) Render without photo (no-photo fallback) → valid PDF.
//   (3) ATS harness: reading order + word boundaries + content.
//   (4) Sample PDFs written to target/ for human review.
//
// A fixture photo is generated in-test via the `image` crate (240×240 solid
// PNG → base64 data URL) — no committed binary needed.

use crate::export::typst_engine::resolve_photo;

/// Generate a 240×240 solid RGBA PNG as a base64 data URL, for use as a
/// fixture photo in the photo-template tests.
fn fixture_photo_data_url() -> String {
    use base64::Engine;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    // Gradient-ish: top-left warm orange, bottom-right deep blue-slate.
    let img = ImageBuffer::from_fn(240, 240, |x, y| {
        let r = (200u8).saturating_sub((x as u8).saturating_mul(170u8 / 240u8));
        let g = (100u8).saturating_add(y as u8 / 3);
        let b = (50u8).saturating_add(x as u8 / 3);
        Rgba([r, g, b, 255])
    });
    let dyn_img = DynamicImage::ImageRgba8(img);
    let mut buf = Vec::new();
    dyn_img
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("fixture_photo: encode png");
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    format!("data:image/png;base64,{b64}")
}

fn opts_photo(ats: bool) -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats,
    }
}

// ── Portrait ──────────────────────────────────────────────────────────────────

// (1a) Portrait with fixture photo → valid PDF.
#[test]
fn portrait_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(portrait) should succeed");

    assert!(!bytes.is_empty(), "Portrait PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Portrait output must start with %PDF"
    );
}

// (1b) Portrait without photo (no-photo fallback) → valid PDF.
#[test]
fn portrait_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, no-photo) should succeed");

    assert!(!bytes.is_empty(), "Portrait no-photo PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1b-multibyte) Portrait's no-photo monogram fallback slices the candidate's
// first name to build initials. A byte-offset `.slice(0, 1)` panics Typst
// whenever the first character is multi-byte in UTF-8 (plausible DACH/EU
// names) — this pins the grapheme-safe fix (`.clusters().first()`).
#[test]
fn portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1c) Portrait ATS mode → valid PDF with linear reading order.
#[test]
fn portrait_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on portrait ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "portrait ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "portrait ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "portrait ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "portrait ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("portrait ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "portrait ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

// (1d) Write Portrait sample PDFs to target/ for human review (with and without photo).
#[test]
fn portrait_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("portrait with photo");
    match fs::write(target.join("portrait_sample.pdf"), &bytes_with) {
        Ok(()) => eprintln!("Portrait (with photo) sample written to target/portrait_sample.pdf"),
        Err(e) => eprintln!("portrait_write: could not write portrait_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));

    // Without photo (no-photo fallback).
    let bytes_nophoto = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo");
    match fs::write(target.join("portrait_nophoto_sample.pdf"), &bytes_nophoto) {
        Ok(()) => {
            eprintln!("Portrait (no-photo) sample written to target/portrait_nophoto_sample.pdf")
        }
        Err(e) => eprintln!("portrait_write: could not write portrait_nophoto_sample.pdf: {e}"),
    }
    assert!(bytes_nophoto.starts_with(b"%PDF"));
}

// ── Lebenslauf ────────────────────────────────────────────────────────────────

/// German Lebenslauf fixture — uses typical DACH names and section content.
const LEBENSLAUF_FIXTURE: &str = "\
Max Müller
max.mueller@example.de | https://linkedin.com/in/maxmueller

BERUFSERFAHRUNG
Senior Software Engineer | Musterfirma GmbH | 2020 – Heute
- Entwicklung einer hochverfügbaren Microservices-Architektur mit Kubernetes
- Einführung von CI/CD-Pipelines und Reduktion der Deployment-Zeit um 60 Prozent

Software Engineer | Tech AG | 2017 – 2020
- Aufbau einer Echtzeit-Datenplattform für zwei Millionen tägliche Nutzer
- Mentoring von drei Junior-Entwicklern im Bereich Rust und TypeScript

AUSBILDUNG
M.Sc. Informatik | Technische Universität Berlin | 2015 – 2017

KENNTNISSE
Rust, Go, TypeScript, Kubernetes, AWS, PostgreSQL, Kafka

SPRACHEN
Deutsch (Muttersprache), Englisch (fließend)
";

// (2a) Lebenslauf with fixture photo → valid PDF.
#[test]
fn lebenslauf_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(lebenslauf) should succeed");

    assert!(!bytes.is_empty(), "Lebenslauf PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Lebenslauf output must start with %PDF"
    );
}

// (2b) Lebenslauf without photo → valid PDF.
#[test]
fn lebenslauf_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, no-photo) should succeed");

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF"));
}

// (2c) Lebenslauf ATS harness.
#[test]
fn lebenslauf_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on lebenslauf ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "lebenslauf ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "lebenslauf ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "lebenslauf ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "lebenslauf ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("lebenslauf ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "lebenslauf ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

// (2c-tier) ATS mode drops the Lebenslauf photo.
//
// Verifies `lebenslauf.typ`'s `#if not is-ats and has-photo` branch through the
// render path: with a real photo supplied, the non-ATS render embeds it (Typst
// emits the raster as an SVG `<image>` element) but the ATS render omits it. The
// SVG emit shares the exact world/data as the PDF path, so this is the cheapest
// reliable assertion — no PDF-internals parsing needed.
#[test]
fn lebenslauf_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → the raster photo is embedded as an SVG <image>.
    let pages_shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, non-ats) should succeed");
    assert!(
        pages_shown.join("").contains("<image"),
        "non-ATS Lebenslauf with a photo must embed it as an <image> element"
    );

    // ATS + the same photo → `#if not is-ats and has-photo` is false → no image.
    let pages_ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, ats) should succeed");
    assert!(
        !pages_ats.join("").contains("<image"),
        "ATS-mode Lebenslauf must drop the photo (no <image> element)"
    );
}

// (2d) Write Lebenslauf sample PDFs to target/ for human review.
#[test]
fn lebenslauf_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("lebenslauf with photo");
    match fs::write(target.join("lebenslauf_sample.pdf"), &bytes_with) {
        Ok(()) => {
            eprintln!("Lebenslauf (with photo) sample written to target/lebenslauf_sample.pdf")
        }
        Err(e) => eprintln!("lebenslauf_write: could not write lebenslauf_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));
}

// ── Aria / Saffron (PR4 design two-column photo templates) ────────────────────
//
// Both are photo-capable two-column templates rendered through bespoke `.typ`
// sources.  Per template we assert: valid PDF with + without a photo (fallback
// path), ATS mode drops the photo (SVG `<image>` assert like Lebenslauf), the
// document-accent override changes the output, `is_two_column` is true, a 2-page
// fixture keeps the sidebar band to page 1, and the per-template placement
// override lands the moved section in the main column.

/// Fixture with distinct EDUCATION + CERTIFICATIONS + SKILLS sections so the
/// per-template placement override can be asserted at the serialized-JSON level.
const PLACEMENT_FIXTURE: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Built a distributed task scheduler

EDUCATION
State University  2013 - 2017
BSc Computer Science

SKILLS
- Rust, Go, TypeScript

CERTIFICATIONS
- AWS Certified Solutions Architect
";

/// Serialized column placement (`"main"` / `"sidebar"`) for the section with the
/// given canonical `kind`, as produced by `prepare` for `template_id`. This is
/// the single substrate that both the PDF and DOCX two-column splits consume.
fn placement_of(template_id: TemplateId, kind: &str) -> String {
    use super::render::{prepare, PreparedRender};
    let model = model_from_resume_text(PLACEMENT_FIXTURE);
    let t = Template::get(template_id);
    let source = TypstTemplate::from_template(&t).source_with_scale();
    let PreparedRender { data_json, .. } =
        prepare(&model, &source, &opts_a4(), Some(&t)).expect("prepare should succeed");
    let v: serde_json::Value =
        serde_json::from_slice(&data_json).expect("data.json must be valid JSON");
    let sections = v["sections"].as_array().expect("sections array");
    let sec = sections
        .iter()
        .find(|s| s["kind"] == kind)
        .unwrap_or_else(|| panic!("section kind {kind:?} not found in {sections:?}"));
    sec["placement"]
        .as_str()
        .expect("placement string")
        .to_string()
}

// ── Aria ────────────────────────────────────────────────────────────────────────

#[test]
fn aria_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(aria) should succeed");
    assert!(!bytes.is_empty(), "Aria PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria output must start with %PDF"
    );
}

#[test]
fn aria_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria no-photo must start with %PDF"
    );
}

#[test]
fn aria_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → embedded as an SVG <image>.
    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("aria non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Aria with a photo must embed it as an <image> element"
    );

    // ATS + same photo → linear, no image.
    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("aria ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Aria must drop the photo (no <image> element)"
    );
}

#[test]
fn aria_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    // Export path linearizes for ATS; replicate it here for the reading-order check.
    crate::model::transform::linearize(&mut model);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("aria ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // ATS linearization (`model::transform::linearize`) reorders `data.sections`
    // to the fixed canonical ATS reading order (Summary, Experience, Skills,
    // Projects, Education, Certifications, Languages, Awards, Publications) — it
    // ignores column `placement` entirely, which only shapes the two-column
    // VISUAL layout and never applies in ATS mode. So the Aria placement override
    // (Education → main column) has no bearing here: the expected order is the
    // canonical ATS order, not the placement-projected one.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "aria ATS reading order wrong (expected canonical ATS order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn aria_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("aria base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("aria accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Aria's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Aria's SVG fills"
    );
}

#[test]
fn aria_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Aria));
}

#[test]
fn aria_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(
        count_pdf_pages(&bytes) >= 2,
        "multi-page fixture must produce ≥2 pages"
    );
    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract")
        .to_lowercase();
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Aria sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn aria_moves_education_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Aria, "education"),
        "main",
        "Aria: Education must be placed in the main column"
    );
    // The rest of the sidebar set is unchanged for Aria.
    assert_eq!(placement_of(TemplateId::Aria, "skills"), "sidebar");
    assert_eq!(placement_of(TemplateId::Aria, "certifications"), "sidebar");
}

// ── Saffron ─────────────────────────────────────────────────────────────────────

#[test]
fn saffron_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(saffron) should succeed");
    assert!(!bytes.is_empty(), "Saffron PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron output must start with %PDF"
    );
}

#[test]
fn saffron_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron no-photo must start with %PDF"
    );
}

// Saffron's no-photo monogram fallback shares Portrait's slicing logic (copied
// pattern) — same grapheme-safety pin as
// `portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names`.
#[test]
fn saffron_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn saffron_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("saffron non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Saffron with a photo must embed it as an <image> element"
    );

    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("saffron ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Saffron must drop the photo (no <image> element)"
    );
}

#[test]
fn saffron_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    crate::model::transform::linearize(&mut model);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("saffron ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // Same semantics as `aria_ats_mode_linearizes_reading_order`: ATS mode uses
    // the canonical ATS order (Summary, Experience, Skills, Projects, Education,
    // Certifications, …) regardless of Saffron's placement override
    // (Certifications → main column), which is visual-only and never applies in
    // ATS mode. Include `education` so the expected order isn't coincidentally
    // satisfied by only checking two of the four sections.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "saffron ATS reading order wrong (expected canonical ATS order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn saffron_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("saffron accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Saffron's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Saffron's SVG fills"
    );
}

#[test]
fn saffron_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Saffron));
}

#[test]
fn saffron_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(
        count_pdf_pages(&bytes) >= 2,
        "multi-page fixture must produce ≥2 pages"
    );
    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract")
        .to_lowercase();
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Saffron sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn saffron_moves_certifications_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Saffron, "certifications"),
        "main",
        "Saffron: Certifications must be placed in the main column"
    );
    // Education stays in the sidebar for Saffron (unlike Aria).
    assert_eq!(placement_of(TemplateId::Saffron, "education"), "sidebar");
    assert_eq!(placement_of(TemplateId::Saffron, "skills"), "sidebar");
}

#[test]
fn portrait_placement_is_unchanged_by_the_refactor() {
    // Control: the default table (Portrait) keeps Education + Certifications in
    // the sidebar — the per-template id parameter must not shift it.
    assert_eq!(placement_of(TemplateId::Portrait, "education"), "sidebar");
    assert_eq!(
        placement_of(TemplateId::Portrait, "certifications"),
        "sidebar"
    );
}

// ── resolve_photo unit tests (already in photo.rs; re-exercised here for ──────
//    integration-layer confidence that the export module re-exports correctly)

#[test]
fn resolve_photo_valid_data_url_returns_png_bytes() {
    let data_url = fixture_photo_data_url();
    let result = resolve_photo(&data_url);
    assert!(
        result.is_some(),
        "resolve_photo must return Some for a valid PNG data URL"
    );
    let bytes = result.unwrap();
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "resolve_photo output must be PNG; got {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn resolve_photo_oversized_returns_none() {
    let huge: String = "A".repeat(15 * 1024 * 1024);
    let data_url = format!("data:image/png;base64,{huge}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "oversized data URL must return None"
    );
}

#[test]
fn resolve_photo_non_image_returns_none() {
    use base64::Engine;
    let garbage = b"not an image at all";
    let b64 = base64::engine::general_purpose::STANDARD.encode(garbage);
    let data_url = format!("data:image/png;base64,{b64}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "non-image bytes must return None"
    );
}

#[test]
fn resolve_photo_bogus_path_returns_none() {
    assert!(
        resolve_photo("/nonexistent/path/photo.png").is_none(),
        "nonexistent path must return None"
    );
}

// ── Stray-Typst-code guard ────────────────────────────────────────────────────
//
// Renders a fixture through EVERY template (classic, swiss-minimal,
// academic, atelier, meridian, throughline, portrait, lebenslauf, letter) and
// asserts that the extracted PDF text contains NONE of the following
// case-sensitive substrings — these are Typst code tokens that would appear as
// literal printed text when a `#` prefix is accidentally omitted from a
// top-level call in markup context.
//
// Caught by this guard:
//   - `line(length` / `stroke:` / `block(above` / `block(below` / `grid(columns`
//   - `#let` / `pad(left` / `place(`
//
// This guard caught the `lebenslauf.typ` Bug 2 (missing `#` before `line` and
// `block` in the header section) and will catch any future regression across
// all templates.

const STRAY_TOKENS: &[&str] = &[
    "line(length",
    "stroke:",
    "block(above",
    "block(below",
    "grid(columns",
    "#let",
    "pad(left",
    "place(",
    "tracking:",
    "smallcaps(",
];

/// Render `bytes` through pdf-extract and assert no stray Typst tokens appear.
fn assert_no_stray_tokens(label: &str, bytes: &[u8]) {
    let extracted = pdf_extract::extract_text_from_mem(bytes)
        .unwrap_or_else(|e| panic!("stray-token guard: pdf-extract failed for {label}: {e}"));

    for token in STRAY_TOKENS {
        assert!(
            !extracted.contains(token),
            "stray-token guard [{label}]: found leaked Typst code token {token:?} \
             in extracted text — a `#` prefix is likely missing in the template source.\n\
             Extracted snippet (first 2000 chars):\n{:.2000}",
            extracted,
        );
    }
}

#[test]
fn stray_typst_code_guard_classic() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("stray-token guard: classic render failed");
    assert_no_stray_tokens("classic", &bytes);
}

#[test]
fn stray_typst_code_guard_swiss_minimal() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("stray-token guard: swiss-minimal render failed");
    assert_no_stray_tokens("swiss-minimal", &bytes);
}

#[test]
fn stray_typst_code_guard_academic() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("stray-token guard: academic render failed");
    assert_no_stray_tokens("academic", &bytes);
}

#[test]
fn stray_typst_code_guard_atelier() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("stray-token guard: atelier render failed");
    assert_no_stray_tokens("atelier", &bytes);
}

#[test]
fn stray_typst_code_guard_meridian() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("stray-token guard: meridian render failed");
    assert_no_stray_tokens("meridian", &bytes);
}

#[test]
fn stray_typst_code_guard_throughline() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("stray-token guard: throughline render failed");
    assert_no_stray_tokens("throughline", &bytes);
}

#[test]
fn stray_typst_code_guard_portrait_with_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("stray-token guard: portrait (with photo) render failed");
    assert_no_stray_tokens("portrait-with-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_portrait_no_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("stray-token guard: portrait (no-photo) render failed");
    assert_no_stray_tokens("portrait-no-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_lebenslauf_with_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("stray-token guard: lebenslauf (with photo) render failed");
    assert_no_stray_tokens("lebenslauf-with-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_lebenslauf_no_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("stray-token guard: lebenslauf (no-photo) render failed");
    assert_no_stray_tokens("lebenslauf-no-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_letter() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(LETTER_FIXTURE_US, &t, None, Some("Jane Smith"), "us", "en")
        .expect("stray-token guard: letter render failed");
    assert_no_stray_tokens("letter", &bytes);
}

// ── PR3: heading_tracking / link_underline / rule_thickness knobs ──────────────
// (backward-compat proof)
//
// Every pre-PR3 template ships heading_tracking: 0.0, link_underline: false, and
// (for the ones whose rule is actually drawn) rule_thickness: 0.5 — the house
// default. By construction:
//   - `heading-run(...)` only emits `tracking: …` when `heading-tracking != 0.0`;
//     at 0.0 it falls through to the exact `text(size:, weight:, fill:, font:,
//     content)` call that existed before the knob (byte-for-byte, verified by
//     reading the branch).
//   - `render-runs(...)` only wraps a link in `underline(…)` when `link-underline`
//     is true; at `false` the branch reduces to the bare `styled` value — the
//     same `link(r.link, text(fill: c-accent, t))` call as before.
//   - the rule stroke resolves `(rule-thickness * 1pt) + c-rule`, and every
//     pre-PR3 ruled template ships `rule_thickness: 0.5` — `0.5 * 1pt == 0.5pt`,
//     the same literal stroke as before. (SwissMinimal ships `0.0`, but its
//     `section_style` is `BoldOnly`, so the ruled-bottom `line(...)` call — the
//     only reader of `rule-thickness` — never executes for it either way.)
//
// Rather than re-deriving that proof at test time by rendering the same code
// path twice and diffing the bytes (tautological — it would always pass), this
// renders ONCE per template and checks two INDEPENDENT anchors decoded from the
// compiled PDF: (a) no stray Typst source token leaked into the extracted text
// (`assert_no_stray_tokens`, which also now guards the new `tracking:` /
// `smallcaps(` syntax), and (b) the known section headings still appear intact,
// in the expected reading order.

#[test]
fn pr3_knob_defaults_leave_pre_pr3_headings_and_content_intact() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
    ] {
        let t = Template::get(id);
        assert_eq!(
            t.heading_tracking, 0.0,
            "{id:?}: heading_tracking must be 0.0"
        );
        assert!(!t.link_underline, "{id:?}: link_underline must be false");

        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| panic!("{id:?}: render failed: {e:?}"));
        assert_no_stray_tokens(&format!("{id:?}-knob-defaults"), &bytes);

        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("{id:?}: pdf-extract failed: {e}"));
        let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = normalised.to_lowercase();

        assert!(
            lower.contains("jane doe"),
            "{id:?}: candidate name missing after knob threading\n---\n{lower}"
        );
        let order = ["summary", "experience", "education", "skills"];
        let mut last = 0usize;
        for h in &order {
            let pos = lower.find(h).unwrap_or_else(|| {
                panic!("{id:?}: heading '{h}' missing after knob threading\n---\n{lower}")
            });
            assert!(
                pos >= last,
                "{id:?}: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
            );
            last = pos;
        }
    }
}

// ── Cadence ───────────────────────────────────────────────────────────────────

#[test]
fn cadence_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Cadence);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(cadence) should succeed");
    assert!(!bytes.is_empty(), "Cadence PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Cadence output must start with %PDF"
    );
}

#[test]
fn cadence_accent_override_applies() {
    let base = Template::get(TemplateId::Cadence);
    let overridden = Template::get(TemplateId::Cadence).with_accent_override(Some("#00AA33"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (0, 170, 51));
    assert_eq!(overridden.emphasis_color, (0, 170, 51));
}

#[test]
fn cadence_tracking_and_underline_change_the_rendered_svg() {
    // Cadence sets heading_tracking 0.08 and link_underline true — prove the
    // knobs actually perturb the rendered SVG (not just config plumbing) by
    // diffing against a neutral (0.0 / false) variant of the same template.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.heading_tracking, 0.08);
    assert!(cadence.link_underline);

    let neutral = Template {
        heading_tracking: 0.0,
        link_underline: false,
        ..Template::get(TemplateId::Cadence)
    };

    let with_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence render should succeed");
    let without_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&neutral),
    )
    .expect("cadence-neutral render should succeed");

    assert_ne!(
        with_knobs, without_knobs,
        "heading_tracking/link_underline must visibly change the rendered SVG"
    );
}

#[test]
fn cadence_rule_thickness_changes_the_rendered_stroke() {
    // Cadence specs a 0.75pt section rule (vs the house 0.5pt default) — prove
    // `rule_thickness` is actually threaded into the rendered stroke width, not
    // just pinned config (the finding this test exists to close: the field was
    // previously dead in every renderer). Isolate it from the other PR3 knobs by
    // holding heading_tracking/link_underline fixed and varying only the stroke.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.rule_thickness, 0.75);

    let half_pt_rule = Template {
        rule_thickness: 0.5,
        ..Template::get(TemplateId::Cadence)
    };

    let with_075 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence (0.75pt rule) render should succeed");
    let with_05 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&half_pt_rule),
    )
    .expect("cadence (0.5pt rule) render should succeed");

    assert_ne!(
        with_075, with_05,
        "rule_thickness must visibly change the rendered SVG stroke"
    );
}

// ── Regent ────────────────────────────────────────────────────────────────────

#[test]
fn regent_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Regent);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(regent) should succeed");
    assert!(!bytes.is_empty(), "Regent PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Regent output must start with %PDF"
    );
}

#[test]
fn regent_accent_override_applies() {
    let base = Template::get(TemplateId::Regent);
    let overridden = Template::get(TemplateId::Regent).with_accent_override(Some("#123456"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (18, 52, 86));
    assert_eq!(overridden.emphasis_color, (18, 52, 86));
}

#[test]
fn regent_maps_to_serif_small_caps_burgundy_style() {
    use super::render::style_from_template;

    let regent = Template::get(TemplateId::Regent);
    let style = style_from_template(&regent);

    // Source Serif 4 throughout, burgundy accent, small-caps (not all-caps)
    // headings, light heading tracking, no link underline.
    assert_eq!(style.font_heading, "Source Serif 4");
    assert_eq!(style.font_name, "Source Serif 4");
    assert_eq!(style.font_body, "Source Serif 4");
    assert!(
        style.section_small_caps,
        "Regent headings must be small-caps"
    );
    assert!(
        !style.section_all_caps,
        "Regent headings must not be all-caps"
    );
    assert_eq!(style.c_accent, "#6E1E2B");
    assert!((style.heading_tracking - 0.04).abs() < f32::EPSILON);
    assert!(!style.link_underline);
    assert_eq!(style.rule_thickness, 0.5);

    // Exercise the actual render path too (not just the JsonStyle mapping) and
    // guard the new `smallcaps(…)` call site — the bundled Source Serif 4 TTF
    // has neither `smcp` nor `c2sc`, and Typst 0.15's `smallcaps` does not yet
    // synthesize small caps for fonts lacking those features, so this renders
    // headings at 0.85× size in their original case rather than visually
    // distinct small-caps glyphs today; `smallcaps(…)` still keeps the PDF text
    // layer's characters unmodified (extraction-safe) and is forward-compatible
    // with a future smcp-capable font swap.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&regent),
    )
    .expect("regent render_pdf should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert_no_stray_tokens("regent-small-caps", &bytes);
}

// ── README showcase banner generator ─────────────────────────────────────────
//
// Renders all twelve templates, rasterises the first page of each at 2× DPI
// (144 px/pt), thumbnails each to 300 px wide, and composes a single wide
// row (1×12) — a banner-proportioned strip like the project hero — on a
// #F4F4F5 background with 20 px border-padding and 14 px gaps, writing the
// result to docs/assets/templates-showcase.png.
//
// As a side output it also writes one per-template preview SVG to
// apps/desktop/src/renderer/features/ai-generate/assets/template-previews/<id>.svg,
// which the AI-Generate option previews show in the result panel. SVG (vector)
// replaces the old PNGs — crisp at any zoom and a fraction of the bundle size.
//
// This test is `#[ignore]`d so it never runs in the normal CI suite.
// Run it explicitly with (the crate is a binary, so target the bin, not --lib):
//   cargo test --bin ajh-tauri -- --ignored generate_templates_showcase_banner
//
// No personal data — synthetic fixture only.  No text-caption rendering dep.

/// Full showcase fixture — richer than FIXTURE_RESUME so templates show premium
/// styling: summary paragraph, multi-entry experience with bullets, skills,
/// education, languages.  Synthetic identity (Alex Carter, example.com contacts).
const SHOWCASE_FIXTURE: &str = "\
Alex Carter
alex.carter@example.com | https://linkedin.com/in/alexcarter | https://alexcarter.dev

SUMMARY
Versatile engineering leader with ten years building high-performance distributed
systems across fintech, healthcare, and cloud infrastructure. Known for bridging
deep technical expertise with product intuition to ship reliable platforms at scale.

EXPERIENCE
Staff Engineer | Apex Technologies | 2021 – Present
- Designed a multi-region event-sourcing platform processing 800 k events per second
- Led architectural review programme adopted by forty backend teams company-wide
- Reduced P99 API latency from 420 ms to 18 ms through adaptive connection pooling
- Mentored six engineers to senior level; two subsequently promoted to staff

Senior Engineer | Meridian Cloud | 2018 – 2021
- Built a zero-downtime schema-migration pipeline managing a 12 TB customer dataset
- Delivered the real-time collaboration layer used by 350 k daily active users
- Cut infrastructure spend by 38 percent via spot-instance scheduling and auto-scaling
- Shipped an internal observability platform reducing mean time-to-resolve by 70 percent

Software Engineer | Cobalt Labs | 2015 – 2018
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Rebuilt the search-indexing pipeline; ingestion lag dropped from six minutes to nine seconds
- Contributed core modules to four open-source libraries with a combined 12 k GitHub stars

PROJECTS
Distributed Rate Limiter | Open Source | 2022
- Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published on crates.io; adopted by twenty organisations within four months of launch

EDUCATION
M.Sc. Computer Science | Westbrook University | 2013 – 2015
B.Sc. Software Engineering | Coastal College | 2009 – 2013

SKILLS
Rust, Go, TypeScript, Python, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform

LANGUAGES
English (native), Spanish (professional), German (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Certified Kubernetes Administrator
";

#[test]
#[ignore]
fn generate_templates_showcase_banner() {
    use image::{DynamicImage, GenericImage, ImageBuffer, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::path::Path;
    use typst_layout::PagedDocument;
    use typst_render::{render as typst_rasterise, RenderOptions};

    use super::engine::TypstTemplate;
    use super::render::{prepare, prepare_with_photo, PreparedRender};
    use super::world::ResumeWorld;
    use crate::export::templates::Template;
    use crate::export::types::TemplateId;
    use crate::locale::PageGeometry;
    use crate::model::adapter::model_from_resume_text;

    // ── Layout constants ──────────────────────────────────────────────────────

    /// Pixels per Typst point at "2×" / 144 dpi.
    /// One Typst point = 1/72 inch → 144 dpi = 2.0 px/pt.
    const PIXEL_PER_PT: f32 = 2.0;

    /// Each thumbnail is scaled to exactly this width (px); height is derived
    /// from the original A4 aspect ratio.
    const CELL_W: u32 = 300;

    /// Layout: a single wide row — 12 columns × 1 row (banner proportions).
    /// Must be >= the template count: `ROWS` is hardcoded to 1, so any template
    /// landing at row-index >= 1 would write pixels beyond `canvas_h` (an
    /// out-of-bounds `put_pixel` panic below). One row keeps the grid math trivial
    /// (`col = idx % COLS`, `row = idx / COLS = 0`) for all twelve templates.
    const COLS: u32 = 12;
    const ROWS: u32 = 1;

    /// Outer border padding (px) and gap between cells (px).
    const PADDING: u32 = 20;
    const GAP: u32 = 14;

    /// Thin 1 px border drawn around each cell (colour: #C8C8CA mid-grey).
    const BORDER: u32 = 1;
    const BORDER_R: u8 = 200;
    const BORDER_G: u8 = 200;
    const BORDER_B: u8 = 202;

    /// Background colour: #F4F4F5 (very light warm grey).
    const BG_R: u8 = 0xF4;
    const BG_G: u8 = 0xF4;
    const BG_B: u8 = 0xF5;

    // ── A4 page geometry for rendering ────────────────────────────────────────

    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    };

    // ── Template list (must be exactly 12, matching the canonical TemplateId set) ──

    // (TemplateId, human label, kebab slug). The slug MUST match the renderer's
    // `TemplateId` wire ids so the per-template preview files line up with the UI.
    let templates: &[(TemplateId, &str, &str)] = &[
        (TemplateId::Classic, "Classic", "classic"),
        (TemplateId::SwissMinimal, "SwissMinimal", "swiss-minimal"),
        (TemplateId::Academic, "Academic", "academic"),
        (TemplateId::Atelier, "Atelier", "atelier"),
        (TemplateId::Meridian, "Meridian", "meridian"),
        (TemplateId::Throughline, "Throughline", "throughline"),
        (TemplateId::Portrait, "Portrait", "portrait"),
        (TemplateId::Lebenslauf, "Lebenslauf", "lebenslauf"),
        (TemplateId::Cadence, "Cadence", "cadence"),
        (TemplateId::Regent, "Regent", "regent"),
        (TemplateId::Aria, "Aria", "aria"),
        (TemplateId::Saffron, "Saffron", "saffron"),
    ];
    assert_eq!(
        templates.len(),
        12,
        "showcase must cover exactly twelve templates"
    );

    // ── Helper: compile a World to a PagedDocument ────────────────────────────

    let compile_world = |world: &ResumeWorld| -> PagedDocument {
        let warned = typst::compile::<PagedDocument>(world);
        for w in &warned.warnings {
            eprintln!("showcase typst warning [{w:?}]");
        }
        warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!("showcase: typst compile error: {}", msg.join("; "));
        })
    };

    // ── Helper: Pixmap → RgbaImage ────────────────────────────────────────────
    //
    // `typst_render::render` returns a `tiny_skia::Pixmap` whose `.data()`
    // is a flat &[u8] in premultiplied RGBA byte order.  Resume templates
    // render on a white background so virtually all pixels are fully opaque
    // (alpha = 255), meaning premultiplied == straight for those pixels.
    // For the handful of anti-aliased edge pixels the visual difference is
    // imperceptible at 420 px thumbnail width, so we copy the raw bytes
    // directly without the overhead of a per-pixel un-premultiply pass.
    // This also avoids a direct `tiny_skia` dev-dependency.

    let pixmap_to_rgba = |pxw: u32, pxh: u32, raw: Vec<u8>| -> RgbaImage {
        RgbaImage::from_raw(pxw, pxh, raw).expect("showcase: pixmap_to_rgba: buffer size mismatch")
    };

    // ── Render + rasterise each template ─────────────────────────────────────

    let model = model_from_resume_text(SHOWCASE_FIXTURE);

    // A4 at 2 px/pt → height of one cell thumbnail.
    // A4: 210 mm wide × 297 mm tall. Typst uses 1pt = 0.352778 mm,
    // so 210 mm = ~595.28 pt → 595.28 * 2 ≈ 1190 px wide before thumbnail.
    // After thumbnail to CELL_W=300: height = 300 * (297/210) ≈ 424 px.
    let a4_aspect = 297.0_f32 / 210.0_f32;
    let cell_h = (CELL_W as f32 * a4_aspect).round() as u32;

    // Per-template preview SVGs for the AI-Generate option previews. Written into
    // the renderer's feature assets (the UI imports them via a Vite glob).
    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all template-previews: {e}"));

    let mut thumbnails: Vec<RgbaImage> = Vec::with_capacity(12);

    for (id, label, slug) in templates {
        eprintln!("showcase: rendering {label}...");

        let t = Template::get(*id);
        let typst_tmpl = TypstTemplate::from_template(&t);
        let source = typst_tmpl.source_with_scale();

        // Photo templates (Portrait, Lebenslauf, Aria, Saffron) take the photo-
        // capable prepare path but render their no-photo fallback so the showcase
        // generator has no binary dependency.
        let has_photo = matches!(
            id,
            TemplateId::Portrait | TemplateId::Lebenslauf | TemplateId::Aria | TemplateId::Saffron
        );

        let PreparedRender {
            source: compiled_source,
            data_json,
        } = if has_photo {
            prepare_with_photo(&model, &source, &opts, Some(&t), false)
                .unwrap_or_else(|e| panic!("showcase: prepare_with_photo({label}) failed: {e}"))
        } else {
            prepare(&model, &source, &opts, Some(&t))
                .unwrap_or_else(|e| panic!("showcase: prepare({label}) failed: {e}"))
        };

        let world = ResumeWorld::with_data(&compiled_source, Some(data_json));
        let document = compile_world(&world);

        assert!(
            !document.pages().is_empty(),
            "showcase: {label} produced zero pages"
        );

        // `render` gained an options parameter in typst 0.15; `pixel_per_pt`
        // moved onto `RenderOptions` (its default is already 2.0 = this scale).
        let render_opts = RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(f64::from(PIXEL_PER_PT)),
            render_bleed: false,
        };
        let pixmap = typst_rasterise(&document.pages()[0], &render_opts);
        let (pxw, pxh) = (pixmap.width(), pixmap.height());
        let raw = pixmap.data().to_vec();
        let rgba = pixmap_to_rgba(pxw, pxh, raw);

        // Per-template preview SVG (vector page-1 export) for the UI picker —
        // crisp at any zoom, a fraction of the old PNG's size, and self-contained
        // (Typst exports glyphs as paths, so there is no font dependency at display time).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            svg.contains("<svg"),
            "showcase: {label} preview SVG missing <svg root element"
        );
        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("showcase: write preview {}: {e}", preview_path.display()));

        // Thumbnail to CELL_W × cell_h.
        let thumb = DynamicImage::ImageRgba8(rgba)
            .thumbnail(CELL_W, cell_h)
            .to_rgba8();

        let (tw_cur, th_cur) = (thumb.width(), thumb.height());
        thumbnails.push(thumb);
        eprintln!("  → thumbnail {tw_cur}×{th_cur}");
    }

    assert_eq!(thumbnails.len(), 12, "must have exactly 12 thumbnails");

    // ── Compose single wide row (1×10) ────────────────────────────────────────

    // Use the actual thumbnail dimensions (thumbnail() preserves aspect, so
    // width should be CELL_W and height close to cell_h).
    let tw = thumbnails[0].width();
    let th = thumbnails[0].height();

    // Canvas size:
    //   width  = PADDING + COLS*(BORDER + tw + BORDER) + (COLS-1)*GAP + PADDING
    //   height = PADDING + ROWS*(BORDER + th + BORDER) + (ROWS-1)*GAP + PADDING
    let canvas_w = PADDING + COLS * (2 * BORDER + tw) + (COLS - 1) * GAP + PADDING;
    let canvas_h = PADDING + ROWS * (2 * BORDER + th) + (ROWS - 1) * GAP + PADDING;

    let bg_pixel = Rgba([BG_R, BG_G, BG_B, 255u8]);
    let border_pixel = Rgba([BORDER_R, BORDER_G, BORDER_B, 255u8]);

    let mut canvas: RgbaImage = ImageBuffer::from_pixel(canvas_w, canvas_h, bg_pixel);

    for (idx, thumb) in thumbnails.iter().enumerate() {
        let col = (idx as u32) % COLS;
        let row = (idx as u32) / COLS;

        // Top-left of the border box for this cell.
        let bx = PADDING + col * (2 * BORDER + tw + GAP);
        let by = PADDING + row * (2 * BORDER + th + GAP);

        // Draw the 1 px border rectangle (top, bottom, left, right edges).
        for x in bx..bx + 2 * BORDER + tw {
            canvas.put_pixel(x, by, border_pixel);
            canvas.put_pixel(x, by + 2 * BORDER + th - 1, border_pixel);
        }
        for y in by..by + 2 * BORDER + th {
            canvas.put_pixel(bx, y, border_pixel);
            canvas.put_pixel(bx + 2 * BORDER + tw - 1, y, border_pixel);
        }

        // Copy thumbnail pixels into the canvas (inside the border).
        let inner_x = bx + BORDER;
        let inner_y = by + BORDER;
        canvas
            .copy_from(thumb, inner_x, inner_y)
            .unwrap_or_else(|e| panic!("showcase: copy_from cell {idx}: {e}"));
    }

    // ── Write PNG ─────────────────────────────────────────────────────────────

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let out_dir = Path::new(manifest_dir).join("../../../docs/assets");

    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all docs/assets: {e}"));

    let out_path = out_dir.join("templates-showcase.png");

    let mut png_buf: Vec<u8> = Vec::new();
    DynamicImage::ImageRgba8(canvas.clone())
        .write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png)
        .unwrap_or_else(|e| panic!("showcase: PNG encode failed: {e}"));

    std::fs::write(&out_path, &png_buf)
        .unwrap_or_else(|e| panic!("showcase: write to {}: {e}", out_path.display()));

    // ── Verify: decode back and check dimensions ──────────────────────────────

    let verified = image::open(&out_path)
        .unwrap_or_else(|e| panic!("showcase: re-open PNG for verification failed: {e}"));

    assert_eq!(
        verified.width(),
        canvas_w,
        "showcase PNG width mismatch after write+re-open"
    );
    assert_eq!(
        verified.height(),
        canvas_h,
        "showcase PNG height mismatch after write+re-open"
    );

    let file_size = png_buf.len();
    assert!(
        file_size >= 80_000,
        "showcase PNG suspiciously small ({file_size} bytes); expected ≥80 KB"
    );
    assert!(
        file_size <= 4_000_000,
        "showcase PNG suspiciously large ({file_size} bytes); expected ≤4 MB"
    );

    eprintln!(
        "templates-showcase.png written: {}×{} px, {} bytes ({} KB)",
        canvas_w,
        canvas_h,
        file_size,
        file_size / 1024,
    );
    eprintln!("  path: {}", out_path.display());

    // ── Verify: all ten per-template previews exist and are non-trivial ───────

    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("showcase: preview {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() >= 1_000,
            "showcase: preview {slug}.svg suspiciously small ({} bytes)",
            meta.len()
        );
    }
    eprintln!(
        "template previews written: 12 SVG → {}",
        preview_dir.display()
    );
}

/// Offline generator: one **cover-letter** style preview per résumé template.
///
/// `#[ignore]`d — an asset generator, not an assertion of behaviour. Run with:
///
/// ```text
/// cargo test --bin ajh-tauri -- --ignored generate_cover_template_previews
/// ```
///
/// This is the cover-letter analog of `generate_templates_showcase_banner`'s
/// per-template previews. For each of the same ten résumé templates it builds
/// the exact cover-letter Typst world that [`super::engine::render_letter_pdf`]
/// produces — `letter_style_from_template` derives the palette + fonts from the
/// résumé [`Template`], so the rendered letter *inherits that template's visual
/// style* — compiles page 1, and exports it to **SVG** (vector, no rasteriser,
/// no `image` crate, no thumbnailing). The ten `.svg` files feed the
/// AI-Generate cover-letter template picker (fetched lazily by the UI via a Vite
/// glob, mirroring the résumé `template-previews/` PNGs).
///
/// Offline hard-wall is respected: all `typst` / `typst_svg` types stay confined
/// to this test fn (same posture as the showcase test, which also imports typst
/// directly) — they never appear in production signatures. `typst-svg` is a
/// dev-dependency, never shipped in the binary.
#[test]
#[ignore]
fn generate_cover_template_previews() {
    use std::path::Path;
    use typst_layout::PagedDocument;

    use super::engine::letter_template_sources;
    use super::letter::{parse_cover_letter, style_from_template as letter_style_from_template};
    use super::world::ResumeWorld;

    // Same twelve templates as the showcase generator. Slugs MUST match the
    // renderer's `TemplateId` wire ids so the preview files line up with the UI.
    let templates: &[(TemplateId, &str, &str)] = &[
        (TemplateId::Classic, "Classic", "classic"),
        (TemplateId::SwissMinimal, "SwissMinimal", "swiss-minimal"),
        (TemplateId::Academic, "Academic", "academic"),
        (TemplateId::Atelier, "Atelier", "atelier"),
        (TemplateId::Meridian, "Meridian", "meridian"),
        (TemplateId::Throughline, "Throughline", "throughline"),
        (TemplateId::Portrait, "Portrait", "portrait"),
        (TemplateId::Lebenslauf, "Lebenslauf", "lebenslauf"),
        (TemplateId::Cadence, "Cadence", "cadence"),
        (TemplateId::Regent, "Regent", "regent"),
        (TemplateId::Aria, "Aria", "aria"),
        (TemplateId::Saffron, "Saffron", "saffron"),
    ];
    assert_eq!(
        templates.len(),
        12,
        "cover previews must cover exactly twelve templates"
    );

    // Embedded letter Typst sources (scale preamble + letter template), reused
    // verbatim from production so the preview matches `render_letter_pdf`.
    let (scale_typ, letter_typ) = letter_template_sources();

    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/cover-template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("cover previews: create_dir_all cover-template-previews: {e}"));

    let mut written = 0usize;

    for (id, label, slug) in templates {
        eprintln!("cover previews: rendering {label}...");

        // Build the letter world exactly like `render_letter_pdf`, inline.
        let t = Template::get(*id);
        let style = letter_style_from_template(&t);
        let model = parse_cover_letter(
            LETTER_FIXTURE_US,
            None,
            Some("Jane Smith"),
            "intl",
            "en",
            style,
        );
        let data_json = serde_json::to_vec(&model)
            .unwrap_or_else(|e| panic!("cover previews: JSON serialise ({label}) failed: {e}"));

        let source = format!(
            "// Auto-generated cover-letter entry — do not edit.\n\
             #let data = json(\"data.json\")\n\
             {scale_typ}\n\
             {letter_typ}"
        );

        let world = ResumeWorld::with_data(&source, Some(data_json));

        // Compile to a PagedDocument (same pattern as the showcase generator).
        let warned = typst::compile::<PagedDocument>(&world);
        for w in &warned.warnings {
            eprintln!("cover previews typst warning [{w:?}]");
        }
        let document = warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!(
                "cover previews: typst compile error ({label}): {}",
                msg.join("; ")
            );
        });

        assert!(
            !document.pages().is_empty(),
            "cover previews: {label} produced zero pages"
        );

        // Export page 1 to SVG (vector — no rasterisation, no thumbnail).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            !svg.is_empty(),
            "cover previews: {label} produced an empty SVG"
        );
        assert!(
            svg.contains("<svg"),
            "cover previews: {label} SVG missing <svg root element"
        );

        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("cover previews: write {}: {e}", preview_path.display()));

        written += 1;
        eprintln!("  → {} ({} bytes)", preview_path.display(), svg.len());
    }

    assert_eq!(
        written, 10,
        "cover previews: expected exactly 10 SVG files written"
    );

    // Verify all ten exist and are non-trivial.
    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("cover previews: {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() > 0,
            "cover previews: {slug}.svg is empty ({label})"
        );
    }
    eprintln!(
        "cover-letter template previews written: 10 → {}",
        preview_dir.display()
    );
}
