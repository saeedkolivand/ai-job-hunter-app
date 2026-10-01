//! Remaining letter-layout pins: Monogram/Banded ATS drops, accent inheritance, per-layout source dispatch.

use super::fixtures::{letter_lower, svg_fill_colors};
use super::letter_fixtures::{LETTER_FIXTURE_LONG_US, LETTER_FIXTURE_US};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{render_letter_pdf, render_letter_svg_pages};

/// Item-2 sanity on the CLASSIC (undecorated) layout: a letterhead-less,
/// date-opening letter with no candidate name still renders a valid PDF and
/// still carries the date and salutation — `data.letterhead.name` is empty
/// (proven directly on the model by `letter.rs`'s
/// `letterhead_name_suppressed_for_date_opening_and_date_still_captured`);
/// this is the end-to-end confirmation that an empty name doesn't break
/// the plainest layout either.
#[test]
fn classic_letter_pdf_renders_with_a_suppressed_date_opening_name() {
    let t = Template::get(TemplateId::SwissMinimal);
    const NO_HEADER_DATE: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let bytes = render_letter_pdf(
        NO_HEADER_DATE,
        &t,
        None,
        None,
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("letterhead-less classic PDF render");
    assert!(bytes.starts_with(b"%PDF"));

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("extract text")
        .to_lowercase();
    assert!(
        lower.contains("12") && lower.contains("march") && lower.contains("2025"),
        "the date must still extract:\n{lower}"
    );
    assert!(
        lower.contains("dear hiring manager"),
        "the salutation must still render:\n{lower}"
    );
}

/// (M3) ATS mode drops Monogram's initials device. Unlike a tint, the device is
/// real TEXT: in design mode extraction reads "js jane smith", two characters of
/// noise ahead of the candidate's actual name, which is exactly what ATS mode
/// exists to remove.
#[test]
fn monogram_device_drops_under_ats_mode_without_losing_words() {
    let design = letter_lower(LetterLayout::Monogram, LETTER_FIXTURE_US, "us", false);
    let ats = letter_lower(LetterLayout::Monogram, LETTER_FIXTURE_US, "us", true);

    assert!(
        design.contains("js jane smith"),
        "design-mode Monogram must render the initials device immediately before the name:\n{design}"
    );
    assert!(
        !ats.contains("js jane smith"),
        "ATS-mode Monogram must NOT emit the initials device — they extract as noise before \
         the name:\n{ats}"
    );
    for needle in [
        "jane smith",
        "jane@example.com",
        "dear hiring manager",
        "distributed systems",
        "sincerely",
    ] {
        assert!(
            ats.contains(needle),
            "ATS-mode Monogram dropped {needle:?} — degradation must lose the device, not words:\n{ats}"
        );
    }
}

/// (B4) The same discipline, applied to the layout that predates it: Banded's
/// accent band is decorative and must go under ATS mode too. Before ATS mode was
/// threaded into the letter path at all, the toggle silently did nothing to a
/// cover letter — a band the user had asked to remove was still exported.
#[test]
fn banded_band_drops_under_ats_mode() {
    let t = Template::get(TemplateId::SwissMinimal);
    let svg = |ats: bool| {
        render_letter_svg_pages(
            LETTER_FIXTURE_US,
            &t,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Banded,
                ats,
            },
        )
        .expect("banded SVG render")
    };
    let design_fills = svg_fill_colors(&svg(false)[0]);
    let ats_fills = svg_fill_colors(&svg(true)[0]);
    assert!(
        !design_fills
            .difference(&ats_fills)
            .collect::<Vec<_>>()
            .is_empty(),
        "the Banded band fill must be present in design mode and absent under ATS mode;\n\
         design={design_fills:?}\nats={ats_fills:?}"
    );
}

// (B3) The Banded accent band is drawn on page 1 ONLY. On a multi-page letter,
// Banded's page-1 SVG carries a filled band colour that (a) the no-band Classic
// layout lacks on its own page 1, and (b) is absent from Banded's later pages.
// typst-svg rasterises the `polygon` as a filled `<path>`, so we detect the band
// by its fill colour rather than a `<polygon>` tag.
#[test]
fn letter_banded_band_draws_on_page_one_only() {
    let t = Template::get(TemplateId::SwissMinimal);
    let banded = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
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
    .expect("banded long-letter SVG render");
    let classic = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
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
    .expect("classic long-letter SVG render");

    assert!(
        banded.len() >= 2,
        "the long fixture must reflow onto ≥2 pages to exercise page-1-only band; got {}",
        banded.len()
    );

    let banded_p1 = svg_fill_colors(&banded[0]);
    let classic_p1 = svg_fill_colors(&classic[0]);
    let banded_last = svg_fill_colors(&banded[banded.len() - 1]);

    // Fills that Banded adds on page 1 relative to the no-band Classic layout —
    // this set contains the decorative band tint.
    let band_only: Vec<&String> = banded_p1.difference(&classic_p1).collect();
    assert!(
        !band_only.is_empty(),
        "Banded page 1 must add a band fill the no-band Classic layout lacks;\n\
         banded p1={banded_p1:?}\nclassic p1={classic_p1:?}"
    );
    // …and that band fill must NOT repeat on later pages (band is page-1 only).
    assert!(
        band_only.iter().any(|f| !banded_last.contains(*f)),
        "the Banded band fill must be absent from later pages;\n\
         band-only p1 fills={band_only:?}\nbanded last-page fills={banded_last:?}"
    );
}

// (P1) Palette inheritance: a non-default résumé template (Regent, burgundy)
// yields different letter output than SwissMinimal for the same layout — the
// template's accent/palette reaches the letter via `style_from_template`.
#[test]
fn letter_layout_inherits_resume_template_accent() {
    let regent = Template::get(TemplateId::Regent);
    let swiss = Template::get(TemplateId::SwissMinimal);

    // Every decorated layout, not just the first two: the palette reaches
    // Sidebar's rail tint and Monogram's device fill through the same
    // `style_from_template` seam, and a hardcoded colour in either would show up
    // here as two identical renders.
    for layout in [
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let a = render_letter_pdf(
            LETTER_FIXTURE_US,
            &regent,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .expect("regent letter render");
        let b = render_letter_pdf(
            LETTER_FIXTURE_US,
            &swiss,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .expect("swiss letter render");
        assert!(
            a != b,
            "{layout:?}: Regent and SwissMinimal must produce different output \
             (accent/palette must reach the letter)"
        );
    }
}

// (D1) EVERY layout dispatches to a distinct source: same data, different bytes.
// Classic remains a valid PDF (default-path regression).
//
// Pairwise over the whole roster rather than three hand-written comparisons —
// the hand-written form is how a new layout ends up silently rendering as
// another one (the DOCX side shipped exactly that bug: Navy rendered as Banded
// because a single boolean sent it down Banded's branch).
#[test]
fn letter_layouts_dispatch_to_distinct_sources() {
    let t = Template::get(TemplateId::SwissMinimal);
    let layouts = [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ];
    let rendered: Vec<(LetterLayout, Vec<u8>)> = layouts
        .iter()
        .map(|&layout| {
            let bytes = render_letter_pdf(
                LETTER_FIXTURE_US,
                &t,
                None,
                Some("Jane Smith"),
                LetterRender {
                    market: "us",
                    lang: "en",
                    layout,
                    ats: false,
                },
            )
            .unwrap_or_else(|e| panic!("{layout:?} render failed: {e}"));
            assert!(
                bytes.starts_with(b"%PDF"),
                "{layout:?} must produce a valid PDF"
            );
            (layout, bytes)
        })
        .collect();

    for (i, (a_id, a)) in rendered.iter().enumerate() {
        for (b_id, b) in rendered.iter().skip(i + 1) {
            assert!(
                a != b,
                "{a_id:?} and {b_id:?} must produce different output"
            );
        }
    }
}
