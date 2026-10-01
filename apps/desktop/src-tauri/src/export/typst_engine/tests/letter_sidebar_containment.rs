//! Letter Sidebar layout rail geometry: measured containment + contact-link survival.

use super::letter_fixtures::LETTER_FIXTURE_US;
use super::pdf_introspect::link_uris;
use super::svg_geometry::{glyph_positions, top_line_extent};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{render_letter_pdf, render_letter_svg_pages};

/// (S6) The rail placement is `place`d with hand-computed offsets
/// (`dx: -(rail-w + rail-gutter - rail-pad)`), so it is measured against the
/// RENDERED page rather than trusted. A sign error or a stale constant would put
/// the letterhead off the left edge or on top of the body, and every text-only
/// assertion above would still pass.
///
/// Geometry under test (`letter_sidebar.typ`): rail 52 mm wide, text inset
/// 7 mm, gutter 10 mm, so the body column starts at 62 mm.
#[test]
fn sidebar_letterhead_is_measurably_inside_the_rail() {
    const MM: f64 = 72.0 / 25.4;
    let rail_pad = 7.0 * MM;
    let rail_text_right = (7.0 + 38.0) * MM;
    // Outer edge of the tinted panel. Used to bound the "rail zone" when
    // measuring type size: the body column's own left edge is NOT safe for
    // that, because a body glyph lands a float-hair below 62mm and would be
    // read as the rail's topmost line, collapsing the measurement to zero.
    let rail_right = 52.0 * MM;
    let body_left = 62.0 * MM;
    let ats_margin = 25.4 * MM;

    // A matrix, because the single "Jane Smith" case fitted the 38 mm block by
    // luck. Every row below is a real name/e-mail length against a template
    // whose `name_pt` is large enough to matter; the long ones overflowed the
    // rail into the gutter and the body column before shrink-to-fit existed.
    // "Alex Li" and "Jane Smith" stay in so the fitter cannot pass by simply
    // shrinking everything.
    // `fits_at_base` marks the rows short enough that the fitter must be a
    // NO-OP: they are the half that makes this test discriminating. Without
    // them, forcing `fit-size` to always return its 6pt floor passes the whole
    // matrix — every assertion here is a glyph POSITION, and shrinking
    // everything only ever produces less overflow, never more.
    let cases: &[(TemplateId, &str, &str, bool)] = &[
        (
            TemplateId::SwissMinimal,
            "Jane Smith",
            "jane@example.com",
            true,
        ),
        (TemplateId::Aria, "Alex Li", "alex@example.com", true),
        (
            TemplateId::Aria,
            "Àlvaro Papadopoulos",
            "alvaro.papadopoulos@example.com",
            false,
        ),
        (
            TemplateId::Cadence,
            "Wojciech Wojciechowski",
            "w.wojciechowski@example.com",
            false,
        ),
        (
            TemplateId::Deedy,
            "Anne Vandenberghe",
            "anne.vandenberghe@example.co.uk",
            false,
        ),
    ];

    for (template_id, name, email, fits_at_base) in cases {
        let t = Template::get(*template_id);
        let profile = crate::contact_profile::ContactProfile {
            full_name: Some((*name).to_string()),
            email: Some((*email).to_string()),
            ..Default::default()
        };
        let page = |ats: bool| {
            render_letter_svg_pages(
                LETTER_FIXTURE_US,
                &t,
                Some(&profile),
                Some(*name),
                LetterRender {
                    market: "us",
                    lang: "en",
                    layout: LetterLayout::Sidebar,
                    ats,
                },
            )
            .unwrap_or_else(|e| panic!("{template_id:?}/{name}: sidebar SVG render: {e}"))[0]
                .clone()
        };

        let design = glyph_positions(&page(false));
        assert!(
            !design.is_empty(),
            "{template_id:?}/{name}: design-mode page 1 rendered no glyphs"
        );

        let leftmost = design
            .iter()
            .map(|(x, _, _)| *x)
            .fold(f64::INFINITY, f64::min);
        assert!(
            (leftmost - rail_pad).abs() < 1.5,
            "{template_id:?}/{name}: the rail text must start exactly at the 7 mm rail              padding ({rail_pad:.1}pt); leftmost glyph is at {leftmost:.1}pt — the `place`              dx arithmetic is off"
        );

        // NOTHING may start between the end of the rail's 38 mm text block and
        // the body column. That span is the rail's right padding plus the 10 mm
        // gutter — it is where an unbreakable token too wide for the block ends
        // up, and it is the only visible symptom, since `place` neither wraps
        // nor clips.
        let overflow: Vec<f64> = design
            .iter()
            .map(|(x, _, _)| *x)
            .filter(|x| *x > rail_text_right + 0.5 && *x < body_left - 0.5)
            .collect();
        assert!(
            overflow.is_empty(),
            "{template_id:?}/{name}: glyphs at {overflow:?} sit past the rail text block              ({rail_text_right:.1}pt) and before the body column ({body_left:.1}pt) — the              letterhead is spilling out of the rail and across the gutter"
        );

        assert!(
            design.iter().any(|(x, _, _)| *x >= body_left - 0.5),
            "{template_id:?}/{name}: no glyph reaches the body column at {body_left:.1}pt —              the widened left margin is not being applied"
        );

        // ATS mode is the full-width single column: no rail, so no fitting, and
        // nothing may sit left of the ordinary margin.
        let ats = glyph_positions(&page(true));
        let ats_leftmost = ats.iter().map(|(x, _, _)| *x).fold(f64::INFINITY, f64::min);
        assert!(
            ats_leftmost >= ats_margin - 1.5,
            "{template_id:?}/{name}: ATS-mode Sidebar put a glyph at {ats_leftmost:.1}pt,              left of the {ats_margin:.1}pt margin — the rail placement is still active"
        );

        // The discriminating half. A name that already fits must render at the
        // rail's BASE size, so the fitter has to leave it alone.
        //
        // Expressed as a ratio against the same name in ATS mode — which always
        // renders at the template's full `name_pt` — so it calibrates itself
        // per template instead of hardcoding point sizes. The rail's base is
        // `name_pt - 4pt`, so the two extents must stand in exactly that ratio.
        // A fitter stuck at its 6pt floor collapses the ratio (6/20 = 0.30
        // against an expected 0.80) and this goes red, while every
        // position-only assertion above still passes it: shrinking everything
        // only ever produces LESS overflow, never more.
        if *fits_at_base {
            let base_ratio = (t.name_pt as f64 - 4.0) / t.name_pt as f64;
            let design_extent = top_line_extent(&design, rail_right);
            let ats_extent = top_line_extent(&ats, f64::INFINITY);
            assert!(
                design_extent > 1.0 && ats_extent > 1.0,
                "{template_id:?}/{name}: measured a degenerate name line \
                 (design={design_extent:.1}pt, ats={ats_extent:.1}pt)"
            );
            let actual = design_extent / ats_extent;
            assert!(
                (actual - base_ratio).abs() < 0.04,
                "{template_id:?}/{name}: the rail name renders at {actual:.3}x the ATS-mode \
                 name, expected {base_ratio:.3}x (= (name_pt - 4)/name_pt). A name this short \
                 already fits the 38 mm rail, so shrink-to-fit must be a NO-OP for it — this \
                 ratio is what separates 'fits what needs it' from 'shrinks everything'."
            );
        }
    }
}

/// (S7) Sidebar's contact line contains real `link()`s, and in design mode the
/// whole letterhead goes through `place()` with a NEGATIVE `dx` into the page
/// margin. Placed-and-negatively-offset content is the documented
/// annotation-loss shape (see the two-column header precedent above), and a
/// dropped `/Annots` entry is invisible to every text assertion — the words
/// still extract, they just stop being clickable.
///
/// Asserted in BOTH modes: design mode is the placed path, ATS mode is the
/// ordinary in-flow path, and only comparing the two shows that placement is
/// what would have cost the annotation.
#[test]
fn sidebar_contact_links_survive_the_placed_rail() {
    let t = Template::get(TemplateId::SwissMinimal);
    let profile = crate::contact_profile::ContactProfile {
        full_name: Some("Jane Smith".to_string()),
        email: Some("jane@example.com".to_string()),
        linkedin: Some("https://linkedin.com/in/janesmith".to_string()),
        ..Default::default()
    };

    for ats in [false, true] {
        let bytes = render_letter_pdf(
            LETTER_FIXTURE_US,
            &t,
            Some(&profile),
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .unwrap_or_else(|e| panic!("sidebar (ats={ats}) render failed: {e}"));

        let uris = link_uris(&bytes);
        assert!(
            uris.iter().any(|u| u.contains("linkedin.com/in/janesmith")),
            "sidebar (ats={ats}): the LinkedIn link annotation was dropped — in design mode the \
             letterhead is `place`d into the margin with a negative dx, which is exactly the \
             shape that loses /Annots. found {uris:?}"
        );
        assert!(
            uris.iter().any(|u| u.contains("jane@example.com")),
            "sidebar (ats={ats}): the mailto link annotation was dropped; found {uris:?}"
        );
    }
}
