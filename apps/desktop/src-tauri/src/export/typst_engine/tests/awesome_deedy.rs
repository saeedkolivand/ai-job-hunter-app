//! Awesome and Deedy bespoke-behavior pins (Phase 8 Track B).

use super::fixtures::{opts_a4, A4_WIDTH_PT};
use super::resume_fixtures::FIXTURE_RESUME;
use super::svg_geometry::{first_filled_rect_bottom, glyph_positions, svg_page1, text_lines};
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, render_resume_svg_pages, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

/// Awesome's design-tier ATS toggle must be more than cosmetic: `ats=true`
/// drops the accent-tinted header band, its keyline, AND the accent-bar
/// section markers, leaving only accent-colored hyperlinks (unaffected by
/// `is-ats`, matching every other non-Classic ATS-safe template). typst-svg
/// renders decorative shapes (the band rect, the keyline, each accent bar) as
/// `<path fill="#…"` / `stroke="#…"` elements, distinct from glyph refs
/// (`<use … fill="#…"`) — the same fill-color detection technique
/// `letter_banded_band_draws_on_page_one_only` uses for the cover-letter band,
/// narrowed to SHAPES so accent-colored link text (unaffected by `is-ats`,
/// same as every other template) can't mask the assertion.
#[test]
fn awesome_ats_mode_drops_the_header_band_and_section_markers() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let template = Template::get(TemplateId::Awesome);
    let accent_hex = format!(
        "#{:02x}{:02x}{:02x}",
        template.accent_color.0, template.accent_color.1, template.accent_color.2
    );
    let fill_needle = format!(r#"<path fill="{accent_hex}""#);
    let stroke_needle = format!(r#"stroke="{accent_hex}""#);
    let count_shapes =
        |svg: &str| svg.matches(&fill_needle).count() + svg.matches(&stroke_needle).count();

    let banded_opts = opts_a4();
    let banded = render_resume_svg_pages(
        &model,
        TypstTemplate::from_template(&template),
        &banded_opts,
        Some(&template),
    )
    .expect("render_resume_svg_pages(awesome, ats=false) should succeed");
    let banded_page1 = banded
        .first()
        .expect("awesome ats=false: at least one page");

    let mut ats_opts = opts_a4();
    ats_opts.ats = true;
    let plain = render_resume_svg_pages(
        &model,
        TypstTemplate::from_template(&template),
        &ats_opts,
        Some(&template),
    )
    .expect("render_resume_svg_pages(awesome, ats=true) should succeed");
    let plain_page1 = plain.first().expect("awesome ats=true: at least one page");

    let banded_shapes = count_shapes(banded_page1);
    let plain_shapes = count_shapes(plain_page1);
    assert!(
        banded_shapes >= 2,
        "awesome (ats=false) must draw at least the band rect + keyline as \
         accent-fill/stroke shapes; got {banded_shapes}"
    );
    // `plain_shapes` isn't zero because the ATS branch draws its OWN decoration:
    // a thin accent-colored `line` under each section heading (`awesome.typ`'s
    // `is-ats` arm; `rule_color == accent_color` in the registry). The non-ATS
    // branch draws no such rule — it draws the band rect, the keyline and one
    // accent bar per section instead — so the two counts are not a subset
    // relation, just strictly ordered. A non-strictly-lower count would mean
    // `is-ats` failed to drop the band/keyline/bars.
    assert!(
        plain_shapes < banded_shapes,
        "awesome (ats=true) must draw fewer decorative accent-fill/stroke shapes \
         than ats=false (band + keyline + section-marker bars must be dropped) — \
         banded={banded_shapes} plain={plain_shapes}"
    );
}

/// Awesome's header is placed inside `page.background`, which lays out at
/// UNBOUNDED width: before `awesome.typ` bounded it to `band-box-w`, a 125-char
/// contact line did not wrap — it ran to x=630pt on a 595pt-wide sheet and the
/// tail was simply not on the page. Bounding it makes it wrap, which only helps
/// while the band is tall enough to hold the wrapped line; otherwise white band
/// ink lands on white paper below the band. Both halves are pinned here against
/// the render, for the two band heights `awesome.typ` budgets (title present or
/// not), because white ink exists ONLY inside the band.
#[test]
fn awesome_band_contains_its_white_header_text() {
    const MM: f64 = 72.0 / 25.4;
    // `awesome.typ`'s `body-margin-h`; the band content shares the body margins.
    let margin = 20.0 * MM;
    let template = Template::get(TemplateId::Awesome);

    for (label, title, band_mm, contact) in [
        (
            "no title, short contact",
            None,
            24.0,
            "jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe",
        ),
        (
            "title + 125-char contact",
            Some("Principal Distributed Systems Engineer"),
            // 28.0 -> 29.0: `band-min-h`'s has-title floor moved with it (#28,
            // see that constant's doc comment) when the name→contact gap
            // below was routed through the taller `sp-name-below`. Then
            // 29.0 -> 31.0 when the name→title gap was routed through
            // `sp-subtitle-gap` (2pt -> 6pt) so the role stopped sitting flush
            // against the name — same failure mode, same constant.
            31.0,
            "alexandra.konstantinopoulos@example.com | +1 (415) 555-0189 | San Francisco, CA \
             | https://linkedin.com/in/alexandrakonst | https://alexandrakonstantinopoulos.dev",
        ),
    ] {
        let mut model = model_from_resume_text(FIXTURE_RESUME);
        model.header.name = "Alexandra Konstantinopoulos".to_string();
        model.header.title = title.map(str::to_string);
        model.header.contact = crate::model::rich::tokenize_rich(contact);

        let svg = svg_page1(&model, &template, false);
        let white: Vec<(f64, f64)> = glyph_positions(&svg)
            .into_iter()
            .filter(|(_, _, fill)| fill.eq_ignore_ascii_case("#ffffff"))
            .map(|(x, y, _)| (x, y))
            .collect();
        assert!(
            white.len() > 20,
            "[{label}] expected the band's white name/contact glyphs, found {}",
            white.len()
        );

        // The band the reader actually sees, measured off the rendered rect —
        // not `band_mm` restated, or shrinking the band would keep this green.
        // `band_mm` only pins that the band stayed THIN (its design brief).
        let accent = format!(
            "#{:02x}{:02x}{:02x}",
            template.accent_color.0, template.accent_color.1, template.accent_color.2
        );
        let band_bottom = first_filled_rect_bottom(&svg, &accent)
            .unwrap_or_else(|| panic!("[{label}] no full-width accent band rect in the render"));

        // Vertical: every white baseline, plus room for its descenders, inside
        // the band. Without the taller band the wrapped contact line lands at
        // y=71.70 against a 68.03pt band bottom — invisible white-on-white.
        let lowest = white.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        assert!(
            lowest + 3.0 <= band_bottom,
            "[{label}] white header text reaches baseline y={lowest:.2} but the band \
             ends at {band_bottom:.2}pt — the overflow renders white-on-white"
        );
        // The band is content-measured (`awesome.typ`'s `#context`), so this is
        // the other half of that rule: for a common 1–2-line header the thin
        // `band-min-h` must still DOMINATE the measurement — the band grows only
        // for genuine overflow (`awesome_band_grows_to_contain_any_contact_line_count`),
        // never creeping wider on ordinary input. Raising `band-pad-bottom` far
        // enough to inflate these two cases fails here.
        assert!(
            (band_bottom - band_mm * MM).abs() < 0.5,
            "[{label}] band is {band_bottom:.2}pt tall, expected the thin {:.2}pt \
             minimum — an ordinary header must not grow the band",
            band_mm * MM
        );

        // No band glyph may be painted in the ACCENT — the band's own fill. The
        // contact line is mostly link runs (LinkedIn / GitHub / Website), and
        // links are the one run kind that carries its own colour: `render-runs`
        // draws them in the accent for the body, `render-runs-white` in the
        // band's white. Both are now one parametrised ladder
        // (`render-runs-in`), and pointing the band at the body's fill paints
        // #c41e3a links onto a #c41e3a band — invisible contact details, passing
        // every other assertion here (the surrounding non-link text stays white,
        // so the white-glyph count and the containment bounds are unmoved).
        let accent_in_band = glyph_positions(&svg)
            .into_iter()
            .filter(|(_, y, fill)| *y <= band_bottom && fill.eq_ignore_ascii_case(&accent))
            .count();
        assert_eq!(
            accent_in_band, 0,
            "[{label}] {accent_in_band} header glyph(s) are painted in the accent \
             {accent} inside the band, which is filled with that same accent — \
             band text (links included) must use the band's white"
        );

        // Horizontal: inside the printable width. Without `box(width: band-box-w)`
        // this reads 630.03 on a 595.28pt page.
        let rightmost = white.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        assert!(
            rightmost <= A4_WIDTH_PT - margin,
            "[{label}] white header text reaches x={rightmost:.2}, past the right \
             margin at {:.2}pt — the placed header is laying out unbounded and \
             running off the sheet",
            A4_WIDTH_PT - margin
        );
    }
}

/// The companion to [`awesome_band_contains_its_white_header_text`]: that test
/// pins the THIN band for the common 1–2-line header, this one pins that the
/// band still contains a header that outgrows it.
///
/// `awesome.typ` used to budget "name + optional title + up to TWO contact
/// lines" as a fixed 24mm/28mm. Nothing caps a header at two lines:
/// `ContactProfile.extra_links` is an unbounded `Vec` and `apply_to_header`
/// copies the whole rendered line into `header.contact`. Built through that real
/// adapter path, a twelve-extra-link profile wraps to THREE lines and put a whole
/// white baseline at y=86.03 against a band ending at 79.37pt — invisible
/// white-on-white ink, and contact details silently gone from the page.
///
/// The fix measures the band from its own content, so this holds for ANY line
/// count, not just three — which is why the assertions below are all relative to
/// the measured band and the measured line count, with nothing restating 24/28mm.
#[test]
fn awesome_band_grows_to_contain_any_contact_line_count() {
    let template = Template::get(TemplateId::Awesome);
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.name = "Alexandra Konstantinopoulos".to_string();
    model.header.title = Some("Principal Distributed Systems Engineer".to_string());
    // The adapter path: a blank contact line filled from the profile.
    model.header.contact = Vec::new();

    let profile = crate::contact_profile::ContactProfile {
        full_name: Some("Alexandra Konstantinopoulos".to_string()),
        email: Some("alexandra.konstantinopoulos@example.com".to_string()),
        phone: Some("+1 (415) 555-0189".to_string()),
        location: Some(crate::contact_profile::LocalizedText {
            default: "San Francisco, California".to_string(),
            by_lang: Default::default(),
        }),
        linkedin: Some("https://linkedin.com/in/alexandrakonst".to_string()),
        github: Some("https://github.com/alexandrakonst".to_string()),
        website: Some("https://alexandrakonstantinopoulos.dev".to_string()),
        extra_links: [
            "Portfolio",
            "Stack Overflow",
            "Google Scholar",
            "Speaker Deck",
            "Dribbble",
            "Behance",
            "Medium",
            "Dev.to",
            "Mastodon",
            "Bluesky",
            "ORCID",
            "Personal Blog",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, label)| crate::contact_profile::ContactLink {
            label: label.to_string(),
            url: format!("https://example.com/alexandra/{i}"),
        })
        .collect(),
        photo: None,
    };
    profile.apply_to_header(&mut model.header, "en");
    assert!(
        !model.header.contact.is_empty(),
        "fixture guard: the profile must have filled the blank contact line"
    );

    let svg = svg_page1(&model, &template, false);
    let white: Vec<(f64, f64)> = glyph_positions(&svg)
        .into_iter()
        .filter(|(_, _, fill)| fill.eq_ignore_ascii_case("#ffffff"))
        .map(|(x, y, _)| (x, y))
        .collect();
    let accent = format!(
        "#{:02x}{:02x}{:02x}",
        template.accent_color.0, template.accent_color.1, template.accent_color.2
    );
    let band_bottom = first_filled_rect_bottom(&svg, &accent)
        .expect("no full-width accent band rect in the render");

    // Fixture guard. The white baselines are name + title + one per wrapped
    // contact line, so fewer than five means the contact did NOT wrap to three
    // lines and this test silently degraded into a copy of the 2-line one —
    // testing the overflow path not at all.
    let mut baselines: Vec<f64> = white.iter().map(|(_, y)| *y).collect();
    baselines.sort_by(f64::total_cmp);
    baselines.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    assert!(
        baselines.len() >= 5,
        "fixture guard: expected name + title + 3+ wrapped contact lines in the \
         band, got {} white baselines ({baselines:?}) — this fixture must exceed \
         the old two-line budget or it tests nothing",
        baselines.len()
    );

    // The band grew past the thin minimum it would otherwise have been pinned at
    // (28mm with a title). Without this, a band that grew to exactly the minimum
    // — i.e. the fix not firing — could still satisfy the containment check on a
    // luckier fixture.
    const MM: f64 = 72.0 / 25.4;
    assert!(
        band_bottom > 28.0 * MM,
        "the band is still {band_bottom:.2}pt — it must grow beyond the 28mm thin \
         minimum to hold a three-line contact"
    );

    let lowest = baselines.last().copied().unwrap_or(f64::MIN);
    assert!(
        lowest + 3.0 <= band_bottom,
        "white header text reaches baseline y={lowest:.2} but the band ends at \
         {band_bottom:.2}pt — the overflow renders white-on-white"
    );

    // Horizontal containment must survive the taller band too.
    let rightmost = white.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
    let margin = 20.0 * MM;
    assert!(
        rightmost <= A4_WIDTH_PT - margin,
        "white header text reaches x={rightmost:.2}, past the right margin at {:.2}pt",
        A4_WIDTH_PT - margin
    );
}

/// Deedy's "generous section spacing" moved out of `deedy.typ` (where it was a
/// local `sp-section-extra = 8pt`, the one template forking `_scale.typ`'s
/// locked rhythm) into `Template::section_above_extra`. The knob has to reach
/// the RENDER, not just sit in the registry: zero it and the first section
/// heading must rise by exactly those 8pt. A field nobody reads moves nothing.
#[test]
fn deedy_section_above_extra_moves_the_rendered_headings() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let deedy = Template::get(TemplateId::Deedy);
    assert_eq!(
        deedy.section_above_extra, 8.0,
        "fixture guard: Deedy is the template carrying the rhythm supplement"
    );
    let mut flat = deedy.clone();
    flat.section_above_extra = 0.0;

    // Line 0 = name, line 1 = contact (this fixture has no title), line 2 = the
    // first section heading — the first thing the supplement pushes down.
    let heading_y = |t: &Template| -> f64 {
        let lines = text_lines(&svg_page1(&model, t, false));
        assert!(lines.len() > 2, "expected a section heading, got {lines:?}");
        lines[2].0
    };

    let with = heading_y(&deedy);
    let without = heading_y(&flat);
    assert!(
        (with - without - 8.0).abs() < 0.1,
        "section_above_extra=8.0 must push the first heading down 8pt: \
         {with:.2} with the knob vs {without:.2} without ({:.2}pt apart)",
        with - without
    );
}

/// `deedy.typ`'s name-block splits `header.name` on the last space to color the
/// surname separately — guarded for a single-token name (`name-tokens.len() <=
/// 1`) that has nothing to split. This is the edge path a naive
/// `.slice(0, len - 1)` would panic on for an empty/underflowing range; render
/// it end-to-end (not just unit-test the split) to prove the guard actually
/// reaches production.
#[test]
fn deedy_single_token_name_does_not_panic() {
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.name = "Cher".to_string();
    let template = Template::get(TemplateId::Deedy);

    for ats in [false, true] {
        let mut opts = opts_a4();
        opts.ats = ats;
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts,
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("deedy single-token name (ats={ats}) should render: {e:?}"));
        assert!(
            bytes.starts_with(b"%PDF"),
            "deedy single-token name (ats={ats}) must start with %PDF"
        );
    }
}

/// #28 regression guard: Awesome's banded header (the exact template in the
/// owner's screenshot) must not cram the contact line against the name.
/// Renders with NO title line (the crammed case — with a title present, the
/// title's own `sp-header-title-below` spacing already dominated), isolates
/// the band's WHITE glyphs (name + contact; body text is dark, per
/// `awesome_band_contains_its_white_header_text`'s established pattern), and
/// measures the baseline gap between the two distinct white lines. Threshold
/// picked ~3pt inside both sides of a direct mutation-check measurement: the
/// unfixed `above: 3pt` gave 9.91pt here, the fixed `sp-name-below` (9pt)
/// gives 15.91pt — both measured by temporarily reverting the constant, not
/// estimated.
///
/// This is a real render assertion (not just "the source references the
/// token") specifically BECAUSE Awesome's band height is itself measured from
/// this same content (`band-min-h`/`measure(band-box)` in `awesome.typ`): a
/// wider gap that silently overflowed the band would still "reference the
/// token" and still be wrong. `awesome_band_contains_its_white_header_text`
/// is the sibling assertion that the (now-taller) band still contains it.
#[test]
fn awesome_name_contact_gap_is_non_trivial_regression() {
    let template = Template::get(TemplateId::Awesome);
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.title = None;
    let svg = svg_page1(&model, &template, false);
    let mut white_y: Vec<f64> = glyph_positions(&svg)
        .into_iter()
        .filter(|(_, _, fill)| fill.eq_ignore_ascii_case("#ffffff"))
        .map(|(_, y, _)| y)
        .collect();
    white_y.sort_by(|a, b| a.total_cmp(b));
    white_y.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    assert_eq!(
        white_y.len(),
        2,
        "expected exactly 2 distinct white baselines (name, contact) with no \
         title line; got {white_y:?}"
    );
    let gap = white_y[1] - white_y[0];
    assert!(
        gap > 13.0,
        "Awesome name→contact baseline gap ({gap:.2}pt) must clear 13.0pt — \
         a regression to a hardcoded `above: 3pt` measures 9.91pt here"
    );
}
