//! `validate_and_fix` end-to-end against real generated PDFs and DOCX: valid documents must
//! not be falsely blocked, on every canonical template, and both formats must carry the
//! same facts.

use super::{support::*, *};
use crate::export::templates::{Template, TemplateTier, CANONICAL_TEMPLATE_IDS};

#[test]
fn single_column_pdf_is_not_blocked() {
    let (bytes, report) = export_pdf(req(ExportFormat::Pdf, TemplateId::SwissMinimal, false));
    assert!(!bytes.is_empty());
    assert!(
        report.ok,
        "a valid single-column resume must export: {:?}",
        report.issues
    );
    assert!(
        report.fixed.is_empty(),
        "no auto-fix expected for single column"
    );
}

#[test]
fn resume_docx_is_not_blocked() {
    let (bytes, report) = validate_and_fix(
        req(ExportFormat::Docx, TemplateId::SwissMinimal, false),
        |r| {
            // `generate_docx` is still on `anyhow::Result`; bridge to the typed error.
            crate::export::docx::generate_docx(r).map_err(crate::error::AppError::from)
        },
    )
    .expect("docx export");
    assert!(!bytes.is_empty());
    assert!(report.ok, "{:?}", report.issues);
}

#[test]
fn two_column_pdf_is_never_blocked() {
    // Atelier is the live two-column template (TwoColumn was deleted).
    let (bytes, report) = export_pdf(req(ExportFormat::Pdf, TemplateId::Atelier, false));
    assert!(!bytes.is_empty());
    assert!(
        report.ok,
        "two-column export must auto-fix rather than block: {:?}",
        report.issues
    );
    // If extraction showed interleaving, the fix linearized to ATS single-column.
    if !report.fixed.is_empty() {
        assert!(
            report.ats_mode,
            "a linearize fix was applied but ats_mode is false"
        );
    }
}

#[test]
fn txt_is_returned_unvalidated() {
    let (bytes, report) = validate_and_fix(
        req(ExportFormat::Txt, TemplateId::SwissMinimal, false),
        |r| Ok(crate::export::parser::strip_md(&r.text).into_bytes()),
    )
    .expect("txt export");
    assert!(!bytes.is_empty());
    assert!(report.ok);
    assert!(report.issues.is_empty());
    assert!(report.fixed.is_empty());
}

// After Cutover-1 every template goes through the Typst engine. The validator
// must not false-positive on a valid Typst PDF (the coordinate-origin and
// text-positioning characteristics of Typst must not produce
// spurious "empty_anchor_link" or "no_extractable_text" criticals).

/// Helper: render via the now-live generate_pdf (Typst) and run validate_and_fix.
fn typst_validate(template_id: TemplateId, ats_mode: bool) -> (Vec<u8>, ExportReport) {
    export_pdf(req(ExportFormat::Pdf, template_id, ats_mode))
}

/// Assert a rendered export cleared the validator outright.
fn assert_validation_clean(id: TemplateId, label: &str, bytes: &[u8], report: &ExportReport) {
    assert!(!bytes.is_empty(), "{id:?} ({label}): empty PDF");
    assert!(
        report.ok,
        "{id:?} ({label}): Typst PDF must pass validate_and_fix — issues: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.severity == Severity::Critical),
        "{id:?} ({label}): no critical issues expected on a valid Typst PDF, got: {:?}",
        report.issues
    );
}

/// Every canonical template must clear the validator, split by column count so
/// the two layout paths stay explicit — `theme::is_two_column` is the same
/// single source of truth the renderer gates on, so the split can't drift from
/// what actually gets rendered.
///
/// Driven off [`CANONICAL_TEMPLATE_IDS`] rather than hardcoded id lists,
/// because the hardcoded ones silently stopped growing: they covered 8 + 4 of
/// twelve-then-sixteen templates, leaving Cologne Navy, Jake, Awesome and Deedy
/// with no validator coverage at all — including Awesome, which emits its
/// contact hyperlinks from inside `page.background`, precisely the annotation
/// shape the `empty_anchor_link` critical looks for.
///
/// **Cost is deliberate.** This test and
/// [`typst_ats_mode_pdf_passes_validation_for_every_toggle_bearing_template`]
/// below together compile 16 + 7 real Typst PDFs on every run — by far the
/// slowest thing in this file. That is the point: the coverage gap above existed
/// precisely *because* someone kept the list short. **Never trim the list to
/// speed it up** — a template dropped from the matrix is a template with no
/// validator coverage, and nothing else will notice.
///
/// The sanctioned mitigation, if the runtime ever genuinely hurts, is to move
/// the whole-roster matrices behind a slower test target (a `#[ignore]`d
/// nightly/CI job, or a separate `--test` binary) that still runs EVERY
/// template — not to sample a subset here.
#[test]
fn typst_every_canonical_template_pdf_passes_validation() {
    let mut single = 0;
    let mut two_col = 0;
    for id in CANONICAL_TEMPLATE_IDS {
        let (bytes, report) = typst_validate(id, false);
        let two_column = crate::theme::is_two_column(id);
        assert_validation_clean(
            id,
            if two_column {
                "two-column"
            } else {
                "single-column"
            },
            &bytes,
            &report,
        );
        if two_column {
            two_col += 1;
        } else {
            single += 1;
        }
    }
    // Both arms must actually be exercised — a `is_two_column` that started
    // answering `false` everywhere would otherwise turn this into a
    // single-column-only test without failing.
    assert!(
        single > 0 && two_col > 0,
        "expected both layout paths to be covered; got {single} single-column \
         and {two_col} two-column templates"
    );
}

/// ATS mode is a second render path (linearized, photo dropped, decorative
/// colour dropped) that the validator must also clear — and it is exactly the
/// path the design tier advertises. Every design-tier template surfaces the
/// toggle (`TemplateTier` doc comment), so that is the set checked here.
#[test]
fn typst_ats_mode_pdf_passes_validation_for_every_toggle_bearing_template() {
    let mut checked = 0;
    for id in CANONICAL_TEMPLATE_IDS {
        if Template::get(id).tier != TemplateTier::Design {
            continue;
        }
        let (bytes, report) = typst_validate(id, true);
        assert_validation_clean(id, "ats", &bytes, &report);
        checked += 1;
    }
    assert!(
        checked >= 6,
        "expected the design tier to surface the ATS toggle on at least six \
         templates; only {checked} were checked"
    );
}

/// The cover-letter path also runs through Typst; validate that it passes.
#[test]
fn typst_cover_letter_pdf_passes_validation() {
    let request = ExportRequest {
        document_type: DocumentType::CoverLetter,
        ..pdf_request(
            "Jane Doe\njane@example.com\n\nDear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe",
            None,
        )
    };
    let (bytes, report) = export_pdf(request);
    assert!(!bytes.is_empty(), "cover letter PDF must not be empty");
    assert!(
        report.ok,
        "Typst cover letter PDF must pass validate_and_fix — issues: {:?}",
        report.issues
    );
}

/// The content every ATS must be able to read back out of an exported résumé,
/// drawn from [`RESUME`] — the same fixture both backends render.
///
/// Anchored on the SOURCE, deliberately. The obvious harness compares the PDF
/// text to the DOCX text, and that is the shape this repo has shipped broken
/// before: two derived values with nothing absolute behind them, so a change
/// that drops a section from BOTH backends keeps the test green while the
/// candidate silently submits a résumé missing their education. Comparing each
/// rendering against the input cannot pass that way.
///
/// Deliberately not the whole fixture verbatim: line breaks, hyphenation,
/// column order and glyph runs legitimately differ between a Typst page and a
/// Word document. What may NOT differ is whether a fact survived.
const PARITY_CONTENT: &[&str] = &[
    "Jane Doe",
    "EXPERIENCE",
    "Acme Corp",
    "Senior Engineer",
    "Led a team of five engineers",
    "SKILLS",
    "Rust",
    "TypeScript",
    "EDUCATION",
    "State University",
    "BSc Computer Science",
];

/// Normalize an extraction for containment checks: collapse whitespace (both
/// backends break lines differently and `strip_xml_tags` injects spaces at
/// every tag boundary) and lowercase.
fn parity_normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// **ADR-002's "golden parity" claim, enforced for the first time.**
///
/// The ADR says the two backends are kept "in golden parity where the design
/// requires, pinned by deterministic golden snapshot tests". The per-backend
/// tests were real, but nothing rendered ONE document through BOTH and checked
/// that the same facts came out — so a backend could silently drop a section
/// and only that backend's own snapshot would notice, if it covered it at all.
///
/// That is not hypothetical for this codebase: DOCX body bold had never
/// rendered at all, and a macOS incident shipped with the two formats
/// disagreeing. An ATS reads the extracted text, so a fact that survives one
/// export and not the other means the candidate submits a materially different
/// résumé depending on the button they pressed.
///
/// Runs the whole canonical roster rather than a sample, for the reason
/// [`typst_every_canonical_template_pdf_passes_validation`] gives: a template
/// nobody rendered is a template nobody validated.
#[test]
fn every_canonical_template_carries_the_same_facts_into_pdf_and_docx() {
    let mut checked = 0;

    for id in CANONICAL_TEMPLATE_IDS {
        let pdf_bytes = crate::export::pdf::generate_pdf(&req(ExportFormat::Pdf, id, false))
            .unwrap_or_else(|e| panic!("{id:?}: pdf export failed: {e}"));
        let docx_bytes = crate::export::docx::generate_docx(&req(ExportFormat::Docx, id, false))
            .unwrap_or_else(|e| panic!("{id:?}: docx export failed: {e}"));

        let pdf = parity_normalize(
            &super::extract_pdf_text(&pdf_bytes)
                .unwrap_or_else(|e| panic!("{id:?}: pdf text extraction failed: {e}")),
        );
        let docx = parity_normalize(
            &super::extract_docx_text(&docx_bytes)
                .unwrap_or_else(|e| panic!("{id:?}: docx text extraction failed: {e}")),
        );

        // Guard the guard: an extractor that silently returns nothing would
        // make every containment check below vacuous.
        assert!(
            pdf.len() > 100 && docx.len() > 100,
            "{id:?}: extraction produced almost nothing (pdf {} chars, docx {} chars) — \
             the parity assertions below would pass vacuously",
            pdf.len(),
            docx.len()
        );

        for fact in PARITY_CONTENT {
            let needle = parity_normalize(fact);
            let in_pdf = pdf.contains(&needle);
            let in_docx = docx.contains(&needle);
            assert!(
                in_pdf && in_docx,
                "{id:?}: {fact:?} survived into {} but not {} — an ATS reads the \
                 extracted text, so the two formats are not the same résumé",
                if in_pdf { "the PDF" } else { "the DOCX" },
                if in_pdf { "the DOCX" } else { "the PDF" }
            );
        }
        checked += 1;
    }

    assert!(
        checked > 0,
        "the canonical roster was empty, so this proved nothing"
    );
}
