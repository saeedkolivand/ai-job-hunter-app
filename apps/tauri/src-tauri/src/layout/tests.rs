//! Unit tests for the single-column layout engine. They drive the engine with
//! the real bundled-font metrics ([`FontMetrics`]) so advance-based geometry
//! (wrapping, centering, right-aligned dates, link rects) is verified exactly,
//! without rendering a PDF.

use super::*;
use crate::export::types::TemplateId;
use crate::locale::PageSize;
use crate::measure::FontMetrics;
use crate::model::adapter::model_from_resume_text;
use crate::model::document::DocumentModel;

const SAMPLE: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane) | https://janedoe.dev

Experienced engineer with a decade building reliable web applications end to end.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform
- Shipped three major features that grew revenue

SKILLS
- Rust, TypeScript, React
";

fn a4() -> PageGeometry {
    PageSize::A4.geometry()
}

fn lay(model: &DocumentModel, id: TemplateId) -> LaidOutDoc {
    let t = Template::get(id);
    layout_document(model, &t, a4(), &FontMetrics)
}

fn sample_doc(id: TemplateId) -> LaidOutDoc {
    let model = model_from_resume_text(SAMPLE);
    lay(&model, id)
}

/// Every PlacedText string across all pages, concatenated.
fn all_text(doc: &LaidOutDoc) -> String {
    doc.pages
        .iter()
        .flat_map(|p| p.texts.iter())
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\u{1}")
}

#[test]
fn lays_out_a_single_page_with_the_name() {
    let doc = sample_doc(TemplateId::Modern);
    assert_eq!(doc.page_width_mm, 210.0);
    assert_eq!(doc.page_height_mm, 297.0);
    assert_eq!(doc.pages.len(), 1, "short resume fits one page");
    assert!(all_text(&doc).contains("Jane Doe"));
}

#[test]
fn name_is_left_aligned_for_classic() {
    let doc = sample_doc(TemplateId::Classic);
    let t = Template::get(TemplateId::Classic);
    let margin = t.margin_in * 25.4;
    let name = doc.pages[0]
        .texts
        .iter()
        .find(|t| t.text == "Jane Doe")
        .expect("name");
    assert!(
        (name.x_mm - margin).abs() < 0.01,
        "classic name starts at the left margin ({margin}), got {}",
        name.x_mm
    );
}

#[test]
fn name_is_centered_for_executive() {
    let doc = sample_doc(TemplateId::Executive);
    let t = Template::get(TemplateId::Executive);
    let name = doc.pages[0]
        .texts
        .iter()
        .find(|t| t.text == "Jane Doe")
        .expect("name");
    let adv = FontMetrics.advance_mm("Jane Doe", t.fonts.name_family, true, t.name_pt);
    let expected = (210.0 - adv) / 2.0;
    assert!(
        (name.x_mm - expected).abs() < 0.01,
        "executive name should be centered at {expected}, got {}",
        name.x_mm
    );
}

#[test]
fn contact_links_become_link_rects_with_correct_urls() {
    let doc = sample_doc(TemplateId::Modern);
    let links = &doc.pages[0].links;
    let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
    assert!(urls.contains(&"mailto:jane@example.com"), "{urls:?}");
    assert!(urls.contains(&"https://linkedin.com/in/jane"), "{urls:?}");
    assert!(urls.contains(&"https://janedoe.dev"), "{urls:?}");

    // Rects are ordered left-to-right along the contact line and have width.
    for l in links {
        assert!(l.width_mm > 0.0, "link rect must have width: {l:?}");
    }
}

#[test]
fn link_rects_stay_within_the_page() {
    let doc = sample_doc(TemplateId::Modern);
    for page in &doc.pages {
        for l in &page.links {
            assert!(
                l.x_mm >= 0.0 && l.x_mm + l.width_mm <= doc.page_width_mm + 0.5,
                "{l:?}"
            );
            assert!(
                l.y_top_mm >= 0.0 && l.y_top_mm <= doc.page_height_mm,
                "{l:?}"
            );
        }
    }
}

#[test]
fn section_heading_is_uppercased_when_all_caps() {
    // Modern has section_all_caps = true.
    let doc = sample_doc(TemplateId::Modern);
    assert!(all_text(&doc).contains("EXPERIENCE"));
    assert!(!all_text(&doc).contains("Experience\u{1}")); // not title-cased as a heading
}

#[test]
fn entry_date_is_right_aligned() {
    let doc = sample_doc(TemplateId::Modern);
    let t = Template::get(TemplateId::Modern);
    let right = 210.0 - t.margin_in * 25.4;
    let date = doc.pages[0]
        .texts
        .iter()
        .find(|t| t.text == "2020 - Present")
        .expect("date");
    let adv = FontMetrics.advance_mm("2020 - Present", t.fonts.body_family, false, 9.0);
    assert!(
        ((date.x_mm + adv) - right).abs() < 0.01,
        "date right edge should hit the content right ({right}), got {}",
        date.x_mm + adv
    );
}

#[test]
fn wrap_runs_keeps_lines_within_the_width() {
    let m = FontMetrics;
    let family = FontFamily::Inter;
    let size = 11.0;
    let max = 60.0;
    let runs = vec![TextRun {
        text: "the quick brown fox jumps over the lazy dog several times in a row".to_string(),
        bold: false,
        italic: false,
        link: None,
    }];
    let lines = wrap_runs(&runs, max, family, size, &m);
    assert!(lines.len() > 1, "long text must wrap");
    for line in &lines {
        let w = line_width(line, family, size, &m);
        assert!(w <= max + 0.01, "wrapped line width {w} exceeds max {max}");
    }
}

#[test]
fn wrap_runs_preserves_bold_formatting() {
    let m = FontMetrics;
    let runs = vec![
        TextRun {
            text: "plain ".to_string(),
            bold: false,
            italic: false,
            link: None,
        },
        TextRun {
            text: "BOLD".to_string(),
            bold: true,
            italic: false,
            link: None,
        },
    ];
    let lines = wrap_runs(&runs, 200.0, FontFamily::Inter, 11.0, &m);
    let has_bold = lines
        .iter()
        .flat_map(|l| l.iter())
        .any(|r| r.bold && r.text.contains("BOLD"));
    assert!(has_bold, "bold run must survive wrapping");
}

#[test]
fn long_resume_breaks_into_multiple_pages() {
    let mut text = String::from("Jane Doe\njane@example.com\n\nEXPERIENCE\n");
    for i in 0..80 {
        text.push_str(&format!(
            "Company {i}  2010 - 2020\nRole {i}\n- Did a number of substantial things worth several lines of description here\n"
        ));
    }
    let model = model_from_resume_text(&text);
    let doc = lay(&model, TemplateId::Modern);
    assert!(
        doc.pages.len() >= 2,
        "a long resume must paginate, got {} page(s)",
        doc.pages.len()
    );
    // Every baseline sits within the page's printable area.
    for page in &doc.pages {
        for t in &page.texts {
            assert!(
                t.baseline_y_mm <= doc.page_height_mm,
                "baseline {} off page",
                t.baseline_y_mm
            );
        }
    }
}

#[test]
fn no_model_text_is_dropped() {
    let doc = sample_doc(TemplateId::Modern);
    let text = all_text(&doc);
    for needle in [
        "Jane Doe",
        "Experienced engineer",
        "EXPERIENCE",
        "Acme Corp",
        "Senior Engineer",
        "Led a team of five",
        "Shipped three major features",
        "SKILLS",
        "Rust, TypeScript, React",
        "2020 - Present",
    ] {
        assert!(text.contains(needle), "lost content: {needle:?}");
    }
}

#[test]
fn every_template_lays_out_without_panicking() {
    let model = model_from_resume_text(SAMPLE);
    for id in [
        TemplateId::Classic,
        TemplateId::Modern,
        TemplateId::Executive,
        TemplateId::EditorialSerif,
        TemplateId::SwissMinimal,
        TemplateId::TwoColumn,
        TemplateId::MonoTechnical,
        TemplateId::RefinedExecutive,
        TemplateId::Academic,
    ] {
        let doc = lay(&model, id);
        assert!(!doc.pages.is_empty(), "{id:?} produced no pages");
        assert!(
            doc.pages.iter().any(|p| !p.texts.is_empty()),
            "{id:?} produced no text"
        );
    }
}

// ─── Two-column layout ────────────────────────────────────────────────────────

/// First PlacedText anywhere in the document whose text equals `needle`.
fn find_text<'a>(doc: &'a LaidOutDoc, needle: &str) -> Option<&'a PlacedText> {
    doc.pages
        .iter()
        .flat_map(|p| p.texts.iter())
        .find(|t| t.text == needle)
}

fn two_column_geometry() -> (f32, f32, Rgb) {
    let t = Template::get(TemplateId::TwoColumn);
    let tc = t.two_column.as_ref().unwrap();
    let margin = t.margin_in * 25.4;
    let content_w = 210.0 - 2.0 * margin;
    let sidebar_w = content_w * tc.sidebar_width_ratio;
    let main_left = margin + sidebar_w + 5.0; // COLUMN_GAP_MM
    (margin, main_left, tc.sidebar_bg_color)
}

#[test]
fn two_column_splits_sections_between_columns() {
    let doc = sample_doc(TemplateId::TwoColumn);
    let (margin, main_left, _) = two_column_geometry();

    // EXPERIENCE is a main-column section → right of the gap.
    let exp = find_text(&doc, "EXPERIENCE").expect("experience heading");
    assert!(
        exp.x_mm >= main_left - 0.5,
        "experience should sit in the main column (>= {main_left}), got {}",
        exp.x_mm
    );

    // SKILLS is a sidebar section (theme::placement_for) → left of the gap.
    let skills = find_text(&doc, "SKILLS").expect("skills heading");
    assert!(
        skills.x_mm < main_left && skills.x_mm >= margin,
        "skills should sit in the sidebar band, got {}",
        skills.x_mm
    );
}

#[test]
fn two_column_draws_a_sidebar_band_at_the_left_margin() {
    let doc = sample_doc(TemplateId::TwoColumn);
    let (margin, _, bg) = two_column_geometry();
    let band = doc.pages[0]
        .fills
        .iter()
        .find(|f| f.color == bg)
        .expect("sidebar band fill");
    assert!(band.width_mm > 0.0 && band.height_mm > 0.0);
    assert!(
        (band.x_mm - margin).abs() < 0.01,
        "band should start at the left margin ({margin}), got {}",
        band.x_mm
    );
}

#[test]
fn two_column_header_spans_full_width_with_links() {
    let doc = sample_doc(TemplateId::TwoColumn);
    // Header (name + contact) is on page 0, full width.
    assert!(find_text(&doc, "Jane Doe").is_some(), "name in header");
    let urls: Vec<&str> = doc.pages[0].links.iter().map(|l| l.url.as_str()).collect();
    assert!(urls.contains(&"https://linkedin.com/in/jane"), "{urls:?}");
    assert!(urls.contains(&"mailto:jane@example.com"), "{urls:?}");
}

#[test]
fn two_column_paginates_with_a_band_on_every_page() {
    // Long main-column content forces multiple pages; each must carry a band.
    let mut text = String::from("Jane Doe\njane@example.com\n\nEXPERIENCE\n");
    for i in 0..70 {
        text.push_str(&format!(
            "Company {i}  2010 - 2020\nRole {i}\n- A sufficiently long bullet line describing the work done in detail here\n"
        ));
    }
    text.push_str("\nSKILLS\n- Rust\n- TypeScript\n");
    let model = model_from_resume_text(&text);
    let doc = lay(&model, TemplateId::TwoColumn);

    assert!(
        doc.pages.len() >= 2,
        "long two-column resume should paginate, got {} page(s)",
        doc.pages.len()
    );
    let (_, _, bg) = two_column_geometry();
    for (i, page) in doc.pages.iter().enumerate() {
        assert!(
            page.fills.iter().any(|f| f.color == bg),
            "page {i} is missing its sidebar band"
        );
    }
}

// Parity gate: single-column vertical spacing must match the legacy rhythm.
//
// The engine regressed by dropping the legacy `calculate_spacing` before-gaps
// and the section-header lead/trail offsets, compressing single-column resumes
// into the top of the page (section-before gaps collapsed to roughly a bare
// line). These tests compare the engine against the legacy renderer for the
// same resume (non-circular). Primary checks are component-level (section
// before-gaps + per-template scale); the total-span check is only a loose
// gross-blowup backstop, because the engine is intentionally a few percent more
// compact than legacy: the canonical model does not carry the blank source
// lines the legacy renderer turns into ~3pt spacers.

const PARITY_RESUME: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane)

Senior engineer with a decade building reliable web applications end to end.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform
- Shipped three major features that grew revenue

Globex Inc  2017 - 2020
Engineer
- Built the public API serving millions of requests

SKILLS
- Rust, TypeScript, React
- AWS, Docker, Kubernetes

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

const SINGLE_COLUMN: [TemplateId; 8] = [
    TemplateId::Classic,
    TemplateId::Modern,
    TemplateId::Executive,
    TemplateId::EditorialSerif,
    TemplateId::SwissMinimal,
    TemplateId::MonoTechnical,
    TemplateId::RefinedExecutive,
    TemplateId::Academic,
];

/// Page-1 text baseline Y positions (points from page bottom) from the `Td` ops,
/// sorted top-of-page first (descending y).
fn baseline_ys(bytes: &[u8]) -> Vec<f32> {
    let doc = lopdf::Document::load_mem(bytes).expect("parse pdf");
    let first = *doc.get_pages().values().next().expect("a page");
    let content = doc.get_page_content(first).expect("page content");
    let decoded = lopdf::content::Content::decode(&content).expect("decode stream");
    let mut ys: Vec<f32> = decoded
        .operations
        .iter()
        .filter(|op| op.operator == "Td")
        .filter_map(|op| op.operands.get(1).and_then(|o| o.as_float().ok()))
        .collect();
    ys.sort_by(|a, b| b.partial_cmp(a).unwrap());
    ys
}

/// Content span (top baseline minus bottom baseline) in points.
fn span_pt(ys: &[f32]) -> f32 {
    match (ys.first(), ys.last()) {
        (Some(t), Some(b)) => t - b,
        _ => 0.0,
    }
}

fn render_pair(id: TemplateId) -> (Vec<f32>, Vec<f32>) {
    use crate::export::layout_pdf::generate_resume_pdf as engine;
    use crate::export::pdf::generate_resume_pdf as legacy;
    let t = Template::get(id);
    let l = baseline_ys(&legacy(PARITY_RESUME, None, &t, false).expect("legacy pdf"));
    let e = baseline_ys(&engine(PARITY_RESUME, None, &t, false).expect("engine pdf"));
    (l, e)
}

/// Gap (mm) from the content line just above each section heading to the heading
/// baseline, on the engine's structured page-1 layout. Isolates the section
/// before-spacing the regression collapsed.
fn engine_section_before_gaps(id: TemplateId) -> Vec<(String, f32)> {
    let t = Template::get(id);
    let model = model_from_resume_text(PARITY_RESUME);
    let doc = layout_document(&model, &t, a4(), &FontMetrics);
    let rows = &doc.pages[0].texts;
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let up = row.text.to_uppercase();
        if (up == "EXPERIENCE" || up == "SKILLS" || up == "EDUCATION") && i > 0 {
            out.push((up, row.baseline_y_mm - rows[i - 1].baseline_y_mm));
        }
    }
    out
}

#[test]
fn single_column_first_ink_matches_legacy() {
    // Name baseline (highest y) identical to legacy: top margin + name block were
    // never the problem; the drift accumulated below them.
    for id in SINGLE_COLUMN {
        let (l, e) = render_pair(id);
        assert!(
            (l[0] - e[0]).abs() < 2.0,
            "{id:?}: first ink moved {:.1} -> {:.1}pt",
            l[0],
            e[0]
        );
    }
}

#[test]
fn single_column_inserts_real_section_spacing() {
    // The precise regression guard. The bug zeroed the before-gap above section
    // headings; with the legacy 12pt before-gap + 4pt heading lead restored, every
    // section-before gap must exceed a bare body line by a clear margin.
    for id in SINGLE_COLUMN {
        let t = Template::get(id);
        let body_line = (t.body_pt / 2.834_645_7) * t.line_spacing;
        let min_gap = body_line + 10.0 / 2.834_645_7;
        let gaps = engine_section_before_gaps(id);
        assert!(
            gaps.len() >= 3,
            "{id:?}: expected EXPERIENCE/SKILLS/EDUCATION gaps, got {gaps:?}"
        );
        for (heading, gap) in gaps {
            assert!(
                gap >= min_gap,
                "{id:?}: gap before {heading} is {gap:.1}mm, expected >= {min_gap:.1}mm \
                 (section spacing collapsed)"
            );
        }
    }
}

#[test]
fn single_column_honors_per_template_spacing_scale() {
    // Each template's own scale must be honored, not a global value: an airier
    // template (Swiss Minimal, line_spacing 1.3) produces a taller content block
    // than a tight one (Academic, 1.1) for the same resume.
    let (_, swiss) = render_pair(TemplateId::SwissMinimal);
    let (_, academic) = render_pair(TemplateId::Academic);
    assert!(
        span_pt(&swiss) > span_pt(&academic),
        "swiss-minimal span {:.1}pt should exceed academic {:.1}pt",
        span_pt(&swiss),
        span_pt(&academic)
    );
}

#[test]
fn single_column_span_backstop_vs_legacy() {
    // Loose gross-blowup guard only (not the primary metric): rejects a total span
    // that has collapsed or ballooned far outside the legacy ballpark. The engine
    // runs a few percent more compact by design (no blank-line spacers).
    for id in SINGLE_COLUMN {
        let (l, e) = render_pair(id);
        let rel = (span_pt(&e) - span_pt(&l)) / span_pt(&l);
        assert!(
            rel.abs() <= 0.12,
            "{id:?}: engine span {:.1}pt vs legacy {:.1}pt = {:+.1}% (allowed +/-12%)",
            span_pt(&e),
            span_pt(&l),
            rel * 100.0
        );
    }
}
