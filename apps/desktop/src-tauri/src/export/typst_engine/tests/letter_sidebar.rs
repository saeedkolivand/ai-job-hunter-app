//! Letter Sidebar layout rail tests (ATS drop, empty letterhead, page repeat).

use super::fixtures::{letter_lower, svg_fill_colors};
use super::letter_fixtures::{LETTER_FIXTURE_LONG_US, LETTER_FIXTURE_US};
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{render_letter_pdf, render_letter_svg_pages};

/// (S3) ATS mode drops Sidebar's tinted rail. Detected the same way the Banded
/// band is: a page-1 fill that design mode has and the ATS render does not.
///
/// The load-bearing second half is that no WORD is lost — a "degradation" that
/// silently drops the contact line would pass a fill-only assertion.
#[test]
fn sidebar_rail_drops_under_ats_mode_without_losing_words() {
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
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .expect("sidebar SVG render")
    };

    let design_fills = svg_fill_colors(&svg(false)[0]);
    let ats_fills = svg_fill_colors(&svg(true)[0]);
    let rail_only: Vec<&String> = design_fills.difference(&ats_fills).collect();
    assert!(
        !rail_only.is_empty(),
        "the Sidebar rail tint must be present in design mode and absent under ATS mode;\n\
         design={design_fills:?}\nats={ats_fills:?}"
    );

    // Same words, both modes — the rail is a position and a tint, not content.
    let design_txt = letter_lower(LetterLayout::Sidebar, LETTER_FIXTURE_US, "us", false);
    let ats_txt = letter_lower(LetterLayout::Sidebar, LETTER_FIXTURE_US, "us", true);
    for needle in [
        "jane smith",
        "jane@example.com",
        "acme corp",
        "dear hiring manager",
        "distributed systems",
        "sincerely",
    ] {
        assert!(
            ats_txt.contains(needle),
            "ATS-mode Sidebar dropped {needle:?} — degradation must lose decoration, not words:\n{ats_txt}"
        );
        assert!(
            design_txt.contains(needle),
            "design-mode Sidebar lost {needle:?}"
        );
    }
}

/// (S3b) Item-2 interaction: when the letterhead is genuinely EMPTY — no
/// candidate name (the fallback landed on a date, refused by
/// `is_letterhead_name`), no attached `ContactProfile`, no title — Sidebar's
/// rail must not paint a pale panel with nothing in it. `show-rail` collapses
/// to the same plain/symmetric treatment ATS mode uses, even though `ats`
/// itself is false here. Detected the same way ATS-mode rail-dropping is: a
/// page-1 fill a named render has that this one must not.
#[test]
fn sidebar_rail_collapses_when_the_letterhead_is_empty() {
    let t = Template::get(TemplateId::SwissMinimal);
    const NO_HEADER_DATE: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let render = |fixture: &str, meta_name: Option<&str>, ats: bool| {
        render_letter_svg_pages(
            fixture,
            &t,
            None,
            meta_name,
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .expect("sidebar SVG render")
    };

    // Precondition: a named letter DOES paint the rail tint, in both design
    // colour sets used below.
    let named_fills = svg_fill_colors(&render(LETTER_FIXTURE_US, Some("Jane Smith"), false)[0]);
    let ats_fills = svg_fill_colors(&render(LETTER_FIXTURE_US, Some("Jane Smith"), true)[0]);
    let rail_tint: Vec<&String> = named_fills.difference(&ats_fills).collect();
    assert!(
        !rail_tint.is_empty(),
        "precondition: a named Sidebar letter must paint a fill the ATS (no-rail) \
         render lacks; named={named_fills:?} ats={ats_fills:?}"
    );

    // The letterhead-less render (no candidate name, date-opening, no
    // ContactProfile) must NOT contain that rail-only fill either, even
    // though `ats` is false — i.e. it must not have painted an empty box.
    let empty_header_fills = svg_fill_colors(&render(NO_HEADER_DATE, None, false)[0]);
    for tint in &rail_tint {
        assert!(
            !empty_header_fills.contains(*tint),
            "a letterhead-less Sidebar page must not paint the rail tint with nothing in it; \
             found {tint:?} in {empty_header_fills:?}"
        );
    }

    // Suppression must lose only the fabricated name, not the words — the
    // date and salutation must still extract normally.
    let bytes = render_letter_pdf(
        NO_HEADER_DATE,
        &t,
        None,
        None,
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: false,
        },
    )
    .expect("letterhead-less sidebar PDF render");
    assert!(bytes.starts_with(b"%PDF"));
    let txt = pdf_extract::extract_text_from_mem(&bytes)
        .expect("extract text")
        .to_lowercase();
    assert!(
        txt.contains("12") && txt.contains("march") && txt.contains("2025"),
        "the date must still extract, just not as the letterhead name:\n{txt}"
    );
    assert!(
        txt.contains("dear hiring manager"),
        "the salutation must still render:\n{txt}"
    );
}

/// (S5) Sidebar keeps its wide left margin and rail on EVERY page (a page-1-only
/// rail would leave page 2 with a 62 mm margin and nothing in it), and the body
/// still flows onto a second page — `place` must not have swallowed the content.
#[test]
fn sidebar_rail_repeats_on_every_page() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: false,
        },
    )
    .expect("sidebar long-letter SVG render");
    assert!(
        pages.len() >= 2,
        "the long fixture must reflow onto ≥2 pages to exercise the rail; got {}",
        pages.len()
    );

    let ats_pages = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: true,
        },
    )
    .expect("sidebar long-letter ATS SVG render");

    // Compared against the ATS render's last page rather than against page 1,
    // so the assertion is "the rail is still here" and not merely "this page has
    // some fill" — every page has text fills.
    let design_last = svg_fill_colors(&pages[pages.len() - 1]);
    let ats_last = svg_fill_colors(&ats_pages[ats_pages.len() - 1]);
    assert!(
        design_last.difference(&ats_last).next().is_some(),
        "the Sidebar rail must be drawn on the LAST page too, not just page 1;\n\
         design last-page fills={design_last:?}\nats last-page fills={ats_last:?}"
    );
}
