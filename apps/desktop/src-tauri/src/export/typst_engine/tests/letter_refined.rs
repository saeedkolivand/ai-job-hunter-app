//! Letter Refined layout render + signature-gap regression tests.

use super::letter_fixtures::{LETTER_FIXTURE_BODY_ONLY_US, LETTER_FIXTURE_US};
use super::svg_geometry::text_lines;
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{render_letter_pdf, render_letter_svg_pages};

// (R1) Refined layout renders a valid US PDF with correct reading order.
#[test]
fn letter_refined_us_renders_valid_pdf() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined US render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined US must start with %PDF"
    );

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined US")
        .to_lowercase();
    assert!(
        lower.contains("dear hiring manager"),
        "salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("distributed systems"),
        "body phrase missing:\n{lower}"
    );
    assert!(lower.contains("sincerely"), "sign-off missing:\n{lower}");
    assert!(
        lower.contains("jane smith"),
        "signature name missing:\n{lower}"
    );

    let pos_sal = lower.find("dear").expect("salutation present");
    let pos_body = lower.find("distributed").expect("body present");
    let pos_signoff = lower.find("sincerely").expect("sign-off present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "refined US reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient inside-address renders (unconditional in Refined) — pin it so a
    // future refactor can't regress it the way Classic silently did.
    assert!(
        lower.contains("123 main street"),
        "refined US: recipient inside address missing:\n{lower}"
    );
    assert!(
        lower.find("acme corp").is_some_and(|p| p < pos_sal),
        "refined US: inside address must read before the salutation:\n{lower}"
    );
}

/// #28/#3 regression guard: the cover-letter sign-off → signature-name
/// baseline gap must be non-trivial. All six letter layouts used to hardcode
/// this independently as a per-layout `v()` literal (20/28/30/34pt); grouped
/// into one `sp-signature-lead`/`sp-signature-gap`-driven non-breakable block
/// now (see the `.typ` files) — pin the measured floor so a future
/// per-layout literal regression can't silently shrink it back unnoticed.
/// `LETTER_FIXTURE_US` closes "Sincerely, / Jane Smith / Software Engineer",
/// so the LAST THREE [`text_lines`] on the (single) page are (sign-off, name,
/// role) in that order — the sign-off→name pair is the third- and
/// second-to-last, not the last two (those are name→role, `sp-subtitle-gap`).
#[test]
fn signature_block_gap_is_non_trivial_regression() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_US,
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
    .expect("render_letter_svg_pages(classic) should succeed");
    let svg = pages.last().expect("at least one page");
    let lines = text_lines(svg);
    assert!(
        lines.len() >= 3,
        "expected at least sign-off + name + role lines, got {} lines",
        lines.len()
    );
    let name = lines[lines.len() - 2];
    let signoff = lines[lines.len() - 3];
    let gap = name.0 - signoff.0;
    assert!(
        gap > 30.0,
        "sign-off→name baseline gap ({gap:.2}pt) must clear 30.00pt — a \
         regression to a small per-layout literal measures lower here"
    );
}

/// Regression guard: `signoff` is optional (a letter can have none), but
/// every layout used to emit `v(sp-signature-gap)` unconditionally — so a
/// letter with no sign-off got an unexplained ~26pt hole before the printed
/// name. Renders the body-only fixture (no salutation/sign-off in the source
/// text, so `parse_cover_letter` leaves `signoff: None`) across every letter
/// layout and asserts the body→name baseline gap stays close to
/// `sp-signature-lead` (20pt) rather than `sp-signature-lead +
/// sp-signature-gap` (46pt).
#[test]
fn no_signoff_letter_has_no_unexplained_signature_gap() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let t = Template::get(TemplateId::SwissMinimal);
        let pages = render_letter_svg_pages(
            LETTER_FIXTURE_BODY_ONLY_US,
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
        .unwrap_or_else(|e| panic!("{layout:?}: render_letter_svg_pages should succeed: {e}"));
        let svg = pages.last().expect("at least one page");
        let lines = text_lines(svg);
        assert!(
            lines.len() >= 2,
            "{layout:?}: expected at least a body line and the signature name, got {} lines",
            lines.len()
        );
        let name = lines[lines.len() - 1];
        let last_body = lines[lines.len() - 2];
        let gap = name.0 - last_body.0;
        assert!(
            gap < 40.0,
            "{layout:?}: body→signature-name baseline gap ({gap:.2}pt) is too \
             wide for a letter with no sign-off — a regression that still \
             emits `sp-signature-gap` unconditionally measures ~46pt here"
        );
    }
}

/// Owner-reported regression (screenshot): `letter_refined`'s header puts the
/// name+role (left, `1fr`) and the contact block (right, `auto`) on the SAME
/// grid row. A long contact block used to render as one long " | "-joined
/// line, claiming enough of the `auto` column's width to squeeze the `1fr`
/// name column into wrapping the candidate's name onto two lines ("Saeed" /
/// "Kolivand" in the screenshot). The contact block now renders one entry
/// per line (`render-runs-stacked`), so the `auto` column is only as wide as
/// its single longest entry. Measures the topmost text line (the name): if
/// it wrapped, that line would only be the first word ("Saeed"), much
/// narrower than the full two-word name.
#[test]
fn letter_refined_header_name_survives_a_long_contact_block_on_one_line() {
    let t = Template::get(TemplateId::SwissMinimal);
    let profile = crate::contact_profile::ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed.kolivand@example.com".to_string()),
        phone: Some("+49 30 1234 5678".to_string()),
        location: Some(crate::contact_profile::LocalizedText {
            default: "Berlin, Germany".to_string(),
            by_lang: Default::default(),
        }),
        linkedin: Some("https://linkedin.com/in/saeedkolivand".to_string()),
        github: Some("https://github.com/saeedkolivand".to_string()),
        website: Some("https://saeedkolivand.dev".to_string()),
        extra_links: [
            "Portfolio",
            "Stack Overflow",
            "Google Scholar",
            "Speaker Deck",
            "Dribbble",
            "Behance",
            "Medium",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, label)| crate::contact_profile::ContactLink {
            label: label.to_string(),
            url: format!("https://example.com/saeed/{i}"),
        })
        .collect(),
        ..Default::default()
    };

    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_US,
        &t,
        Some(&profile),
        Some("Saeed Kolivand"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("render_letter_svg_pages(refined) should succeed");

    let svg = pages.first().expect("at least one page");
    let lines = text_lines(svg);
    assert!(!lines.is_empty(), "expected at least one text line");
    // The name sits in the LEFT (name+role) grid column, which starts flush
    // at the page margin (x=72pt for this template's 25.4mm margin) — filter
    // to that column specifically rather than "topmost line", because a long
    // enough stacked contact list on the RIGHT can be taller than the left
    // column and, `horizon`-centred, extend above it.
    const LEFT_MARGIN_PT: f64 = 72.0;
    let name_line = lines
        .iter()
        .filter(|l| (l.1 - LEFT_MARGIN_PT).abs() < 1.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .expect("expected a left-column text line (the name)");
    let name_width = name_line.2 - name_line.1;
    assert!(
        name_width > 90.0,
        "candidate name line measures only {name_width:.2}pt wide — too narrow \
         for the full two-word name \"Saeed Kolivand\", indicating it wrapped \
         onto a second line (all lines: {lines:?})"
    );
}
