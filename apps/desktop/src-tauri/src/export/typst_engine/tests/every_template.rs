//! Tests that iterate every canonical template (valid PDF, accented-Latin extraction, centring, ATS reading order, project tech-stack line).

use super::fixtures::{
    canonical_template_ids, normalize_like_validator, opts_a4, A4_WIDTH_PT,
    NO_EXTRACTABLE_TEXT_THRESHOLD,
};
use super::pdf_introspect::count_pdf_pages;
use super::resume_fixtures::{ACCENTED_RESUME_FIXTURE, FIXTURE_RESUME};
use super::svg_geometry::{svg_page1, text_lines};
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

#[test]
fn every_template_renders_a_valid_pdf() {
    let ids = canonical_template_ids();
    assert_eq!(ids.len(), 16, "expected the sixteen canonical templates");

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

// Every canonical template must round-trip accented-Latin content — grave
// lowercase + capital È/À, the shape the `no_extractable_text` incident audit
// flagged as under-tested — through `pdf_extract`, not just emit a %PDF
// header. `every_template_renders_a_valid_pdf` above never calls
// `pdf_extract` at all, so a broken/missing ToUnicode CMap on a subset font
// (renders fine on screen, extracts to nothing) would pass it silently; this
// is the regression test for exactly that class of bug.
#[test]
fn every_template_extracts_accented_latin_content() {
    let model = model_from_resume_text(ACCENTED_RESUME_FIXTURE);
    for id in canonical_template_ids() {
        let template = Template::get(id);
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts_a4(),
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("render_pdf({id:?}) should succeed: {e:?}"));

        let extracted = pdf_extract::extract_text_from_mem(&bytes).unwrap_or_else(|e| {
            panic!("{id:?}: pdf-extract must succeed on rendered output: {e:?}")
        });
        let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = normalised.to_lowercase();

        assert!(
            lower.contains("àlvaro") && lower.contains("èsposito"),
            "{id:?}: accented name missing — capitals È/À did not survive extraction\n---\n{extracted:?}"
        );
        assert!(
            lower.contains("così") || lower.contains("però") || lower.contains("città"),
            "{id:?}: grave-accented-lowercase body word missing\n---\n{extracted:?}"
        );

        // Same gate `validate::mod::evaluate` uses to raise the CRITICAL
        // `no_extractable_text` issue — a passing assertion here means the
        // real validator would NOT have blocked this export.
        let normalized_len = normalize_like_validator(&extracted).len();
        assert!(
            normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
            "{id:?}: only {normalized_len} normalized chars extracted — the real \
             validator's no_extractable_text gate (< {NO_EXTRACTABLE_TEXT_THRESHOLD}) \
             would block this export; got {extracted:?}"
        );
    }
}

/// `single_column.typ`'s locked page margin (25.4 mm) in points. Left-aligned
/// header lines start exactly here.
const SINGLE_COLUMN_MARGIN_PT: f64 = 72.0;

/// `Template::name_centered` must be a fact about the RENDERED page, not just a
/// registry field. `single_column.typ` centres with `align(center, …)`, which
/// does nothing inside an `auto`-width block (the block shrinks to its content,
/// so there is no slack to centre in): Jake shipped `name_centered: true` while
/// its name rendered at x=72.0 — the left margin, byte-identical in position to
/// the `name_centered: false` templates. `jake_matches_spec` passed throughout,
/// because a field pin cannot see the layout.
///
/// Midpoints are computed from glyph ORIGINS, so they sit half of the last
/// glyph's advance left of the true visual centre; the tolerance covers that.
/// The bug's signature was ~186pt off centre, so it has an enormous margin.
#[test]
fn name_centered_actually_centres_the_rendered_header() {
    let centre = A4_WIDTH_PT / 2.0;
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.title = Some("Senior Software Engineer".to_string());

    let jake = Template::get(TemplateId::Jake);
    assert!(
        jake.name_centered,
        "fixture guard: Jake is this test's centred single-column case"
    );
    let lines = text_lines(&svg_page1(&model, &jake, false));
    assert!(
        lines.len() >= 3,
        "expected at least name/title/contact lines, got {lines:?}"
    );
    for (label, (y, lo, hi)) in ["name", "title", "contact"]
        .into_iter()
        .zip(lines.iter().copied())
    {
        let mid = (lo + hi) / 2.0;
        assert!(
            (mid - centre).abs() < 15.0,
            "jake's {label} line is not centred: y={y:.2} x=[{lo:.2}..{hi:.2}] \
             midpoint {mid:.2} vs page centre {centre:.2}"
        );
    }

    // Control: the same three lines on a `name_centered: false` template must
    // still start exactly on the left margin. This is what makes the assertion
    // above about CENTRING rather than about "the header moved".
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Cadence,
        TemplateId::Regent,
    ] {
        let t = Template::get(id);
        assert!(
            !t.name_centered,
            "{id:?}: this control list is the left-aligned single-column set"
        );
        for (y, lo, hi) in text_lines(&svg_page1(&model, &t, false))
            .into_iter()
            .take(3)
        {
            assert!(
                (lo - SINGLE_COLUMN_MARGIN_PT).abs() < 0.01,
                "{id:?}: header line y={y:.2} x=[{lo:.2}..{hi:.2}] must stay flush \
                 to the {SINGLE_COLUMN_MARGIN_PT}pt left margin — the centring fix \
                 must not leak to left-aligned templates"
            );
        }
    }
}

/// A Projects section in the locked signature `pipeline::resume::project_render`
/// emits: bold name + link, a `·`-separated tech-stack line, then prose. Kept
/// separate from `FIXTURE_RESUME` on purpose — that fixture is shared by
/// page-count and layout assertions, and growing it would move them.
const PROJECTS_FIXTURE: &str = "\
Jane Doe
jane@example.com | https://github.com/janedoe

PROJECTS

**Ledger CLI** · https://github.com/janedoe/ledger
Rust · SQLite · Clap
A double-entry bookkeeping tool for the terminal.

**Atlas** · https://atlas.example.dev
TypeScript · React · Vite
Framework-agnostic component library published to npm.
";

/// The tech-stack line rides the entry SUBTITLE slot, which every template
/// already styles. This is the guard that the adapter's regrouping actually
/// reaches rendered output on all 16 templates — a template that dropped or
/// never rendered `subtitle` would lose the candidate's technology list
/// silently, and no `%PDF`-header or page-count check would notice.
#[test]
fn every_template_renders_the_project_tech_stack_line() {
    let model = model_from_resume_text(PROJECTS_FIXTURE);
    for id in canonical_template_ids() {
        let template = Template::get(id);
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts_a4(),
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("render_pdf({id:?}) should succeed: {e:?}"));

        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("{id:?}: pdf-extract must succeed: {e:?}"));
        let lower: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = lower.to_lowercase();

        assert!(
            lower.contains("ledger cli") && lower.contains("atlas"),
            "{id:?}: project names missing\n---\n{extracted:?}"
        );
        // Both stacks, so a template that renders only the FIRST entry's
        // subtitle cannot pass.
        assert!(
            lower.contains("rust") && lower.contains("sqlite") && lower.contains("clap"),
            "{id:?}: first project's tech stack missing\n---\n{extracted:?}"
        );
        assert!(
            lower.contains("typescript") && lower.contains("react") && lower.contains("vite"),
            "{id:?}: second project's tech stack missing\n---\n{extracted:?}"
        );
        assert!(
            lower.contains("double-entry bookkeeping"),
            "{id:?}: project description missing\n---\n{extracted:?}"
        );
    }
}
