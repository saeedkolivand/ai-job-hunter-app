//! Shared render-option builders + small helpers/constants reused across the typst_engine test topics.

use super::resume_fixtures::PLACEMENT_FIXTURE;
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{render_letter_pdf, RenderOpts, TypstTemplate};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;

pub(super) fn opts_a4() -> RenderOpts {
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

/// Reading-order assertion shared by the ATS harnesses that use this exact panic
/// wording (SwissMinimal, Academic, Portrait, Lebenslauf — Meridian/Throughline/
/// the PR3-knob-defaults harness each carry their own differently-worded variant,
/// so they keep their own inline loop rather than call this).
pub(super) fn assert_reading_order(label: &str, lower: &str, order: &[&str]) {
    let mut last = 0usize;
    for h in order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("{label} ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "{label} ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

/// Canonical user-facing template set — must match the `TemplateId` enum
/// (pinned by the serde round-trip test in types.rs and the TS sync guard).
/// Shared by every test that iterates "all templates" so a newly added
/// template is covered automatically rather than needing a remembered edit.
///
/// Now a thin alias over `templates::CANONICAL_TEMPLATE_IDS`: the validator
/// matrices in `validate/tests.rs` iterate the same list, so a new template
/// cannot be covered here and silently skipped there.
pub(super) fn canonical_template_ids() -> [TemplateId; 16] {
    crate::export::templates::CANONICAL_TEMPLATE_IDS
}

/// The part of an extracted cover letter AFTER the sign-off, i.e. the signature
/// block. Both letter fixtures print the candidate's name twice (letterhead and
/// signature), so a whole-document `contains` cannot tell "the signature
/// extracted" from "only the letterhead extracted". Returns `""` when the
/// sign-off itself is missing, which fails the caller's assertion — correct,
/// since a letter whose "Sincerely" did not extract is already broken.
pub(super) fn signature_block(lowercased: &str) -> &str {
    lowercased
        .split_once("sincerely")
        .map(|(_, tail)| tail)
        .unwrap_or("")
}

/// Mirrors `validate::mod::normalize` (validate/mod.rs:874-882): lowercased,
/// whitespace-collapsed, alphanumeric-only text used for tolerant `contains`
/// checks. Duplicated here (rather than exposed as `pub(crate)`) because this
/// test file owns no production code — see [`NO_EXTRACTABLE_TEXT_THRESHOLD`].
pub(super) fn normalize_like_validator(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The exact threshold `validate::mod::evaluate` uses (validate/mod.rs:796) to
/// raise the CRITICAL `no_extractable_text` issue that blocks an export: fewer
/// than this many [`normalize_like_validator`]-normalized characters means the
/// document has (almost) no extractable text. Asserting against this constant
/// — not just "some text extracted" — means a passing test proves the real
/// validator would not have blocked the export.
pub(super) const NO_EXTRACTABLE_TEXT_THRESHOLD: usize = 20;

/// A4 width in Typst points — the page every `opts_a4()` render uses.
pub(super) const A4_WIDTH_PT: f64 = 595.275_590_551;

/// [`opts_a4`] with Atelier's custom accent override and the requested ATS mode.
pub(super) fn opts_atelier(ats: bool) -> RenderOpts {
    RenderOpts {
        accent: Some("#4A4580".to_string()),
        ats,
        ..opts_a4()
    }
}

//
// For each new template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).

pub(super) fn template_style(id: TemplateId) -> Template {
    Template::get(id)
}

/// Byte-identical to [`opts_a4`] (kept as its own name: every SingleColumn-family
/// call site reads `opts_sc()`, naming the parametric template it renders).
pub(super) fn opts_sc() -> RenderOpts {
    opts_a4()
}

/// Distinct `fill="#…"` colours present in an SVG string.
pub(super) fn svg_fill_colors(svg: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let needle = "fill=\"";
    let mut rest = svg;
    while let Some(pos) = rest.find(needle) {
        rest = &rest[pos + needle.len()..];
        if let Some(end) = rest.find('"') {
            out.insert(rest[..end].to_string());
        }
    }
    out
}

//
// Both are decorated layouts, so each needs three separate guarantees:
//   (a) the words still come out, in reading order (the ATS harness);
//   (b) the decoration is DROPPED under `data.opts.ats` without losing a word;
//   (c) structural elements gate on `data.opts` (market conventions), never on
//       the layout id.

/// Layouts that must never emit a hyphenated line break into the PDF text
/// layer, i.e. the ones that set `hyphenate: false`. Now the full six-layout
/// roster — Classic/Refined/Banded/Navy picked up the flag in the same change
/// that extended this const (Sidebar and Monogram already had it from Phase
/// 8). Still an explicit roster rather than "every `LetterLayout`" so a future
/// layout that forgets the flag is a missing-test gap to notice, not a
/// silently-passing wildcard.
pub(super) const NO_SOFT_HYPHEN_LAYOUTS: [LetterLayout; 6] = [
    LetterLayout::Classic,
    LetterLayout::Refined,
    LetterLayout::Banded,
    LetterLayout::Navy,
    LetterLayout::Sidebar,
    LetterLayout::Monogram,
];

/// Render + extract + whitespace-normalise + lowercase, the shape every letter
/// assertion below wants. Panics with the layout name so a failure says which.
///
/// Also the single choke point for the SOFT-HYPHEN guard: every extraction in
/// this file flows through here, so a layout in [`NO_SOFT_HYPHEN_LAYOUTS`]
/// cannot regress into hyphenated line breaks via any test, not just a
/// dedicated one. A U+00AD in the extracted text means the PDF really did break
/// the word — "microservices architecture" comes out as "architec­ture" and an
/// ATS tokenising on whitespace loses the keyword.
///
/// Only the SOFT hyphen is checked. The critic's correction to my first
/// measurement: of the three U+00AD-adjacent breaks I counted, only one is a
/// genuine soft-hyphen break — the others are HARD hyphens ("end-to-end"),
/// which Typst tags with `/ActualText` and are recoverable by a conforming
/// extractor. Asserting on the hard ones would be asserting on a non-defect.
pub(super) fn letter_lower(layout: LetterLayout, fixture: &str, market: &str, ats: bool) -> String {
    let t = Template::get(TemplateId::SwissMinimal);
    let name = if market == "de" {
        "Max Müller"
    } else {
        "Jane Smith"
    };
    let lang = if market == "de" { "de" } else { "en" };
    let bytes = render_letter_pdf(
        fixture,
        &t,
        None,
        Some(name),
        LetterRender {
            market,
            lang,
            layout,
            ats,
        },
    )
    .unwrap_or_else(|e| panic!("{layout:?} (ats={ats}) render failed: {e}"));
    assert!(
        bytes.starts_with(b"%PDF"),
        "{layout:?} (ats={ats}) must start with %PDF"
    );
    let txt = pdf_extract::extract_text_from_mem(&bytes)
        .unwrap_or_else(|e| panic!("pdf-extract on {layout:?} (ats={ats}): {e}"))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if NO_SOFT_HYPHEN_LAYOUTS.contains(&layout) {
        assert!(
            !txt.contains('\u{00AD}'),
            "{layout:?} (ats={ats}) emitted a soft hyphen — a hyphenated line break splits a \
             word in the PDF text layer, so an ATS tokenising on whitespace loses the keyword. \
             `hyphenate: false` must stay set on this layout.\n{txt}"
        );
    }
    txt
}

//
// For each template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).
//
// For Throughline additionally:
//   (d) EXPERIENCE entries + bullets all survive extraction (timeline decoration
//       must not drop any text).

/// Byte-identical to [`opts_a4`] (kept as its own name for the Phase 3a
/// premium single-column call sites).
pub(super) fn opts_p3a() -> RenderOpts {
    opts_a4()
}

/// Generate a 240×240 solid RGBA PNG as a base64 data URL, for use as a
/// fixture photo in the photo-template tests.
pub(super) fn fixture_photo_data_url() -> String {
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

/// [`opts_a4`] with the requested ATS mode.
pub(super) fn opts_photo(ats: bool) -> RenderOpts {
    RenderOpts { ats, ..opts_a4() }
}

/// Serialized column placement (`"main"` / `"sidebar"`) for the section with the
/// given canonical `kind`, as produced by `prepare` for `template_id`. This is
/// the single substrate that both the PDF and DOCX two-column splits consume.
pub(super) fn placement_of(template_id: TemplateId, kind: &str) -> String {
    use super::super::render::{prepare, PreparedRender};
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
