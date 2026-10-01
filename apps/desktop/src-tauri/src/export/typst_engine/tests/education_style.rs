//! JsonSection.kind + emphasize_education + entry-spacing style regression tests.

use super::fixtures::{canonical_template_ids, template_style};
use super::svg_geometry::{svg_page1, text_lines};
use crate::export::types::TemplateId;
use crate::model::adapter::model_from_resume_text;

//
// These tests verify the two new data-model fields without needing a PDF render:
//
//   (a) section_id_to_kind: education section serializes as kind == "education".
//   (b) style_from_template: emphasize_education == true for every template
//       (#28 — de-emphasized education titles were a bug, not a design choice;
//       see `render.rs`'s `style_from_template` doc comment).

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

/// #28 regression guard for `render.rs`'s `style_from_template`: a
/// de-emphasized education title was a bug (the owner's report), not a
/// per-template design choice, so EVERY template must now report
/// `emphasize_education == true` — not just Academic (the pre-fix behavior
/// this test used to pin: see git history for the `t.id ==
/// TemplateId::Academic` version). Spans the full canonical roster so a
/// regression to that per-template check fails here regardless of which
/// template it re-excludes.
#[test]
fn style_from_template_emphasize_education_true_for_every_template() {
    use crate::export::typst_engine::render::style_from_template;

    for id in canonical_template_ids() {
        let style = style_from_template(&template_style(id));
        assert!(
            style.emphasize_education,
            "{id:?}: emphasize_education must be true — education entry titles \
             must render bold in every template, not just Academic"
        );
    }
}

/// #28 regression guard, one level deeper than the unit test above: a
/// de-emphasized education title is only OBSERVABLE in the rendered output,
/// not in `data.style` — `single_column.typ`'s `entry-bold-for-section` reads
/// `emphasize_education` itself, so a bug in that Typst-side branch (as
/// opposed to `style_from_template`) would pass the unit test above and still
/// under-bold education. Typst emits glyphs as `<use>` references to path
/// outlines with no font-weight attribute (see `text_lines`'s doc comment on
/// why every geometry test here works off positions, not text), so "same
/// weight" is measured the only way actually observable from the render:
/// bold glyphs of an identical string at an identical size have a different
/// (wider, in every bundled font) total advance width than regular glyphs.
/// Renders two single-entry fixtures (no subtitle/bullets, so the entry
/// title+date grid row is the last thing on the page) through SwissMinimal —
/// the exact template the retired test asserted `false` for — one under
/// EXPERIENCE (always bold) and one under EDUCATION (the one that used to be
/// regular), and compares the measured width of that line between them.
/// SAME title text and SAME date in both (an 11-char single "word" — the
/// parser's `JobEntry` shape needs `word_count >= 2 || left.len() > 10`, so a
/// shorter/thinner fixture silently falls through to a plain paragraph and
/// this test would pass vacuously; caught by the mutation check, not by
/// inspection).
#[test]
fn education_entry_title_renders_same_weight_as_experience_entry_title() {
    // Identical date string in both fixtures: the entry-title grid row puts
    // title and date on the SAME baseline (`single_column.typ`'s
    // `render-entry`), so a differing date string would confound the width
    // comparison with its own length, not just the title's weight.
    const EXPERIENCE_ONLY: &str = "\
Jane Doe

EXPERIENCE
Wxwxwxwxwxw  2020 - Present
";
    const EDUCATION_ONLY: &str = "\
Jane Doe

EDUCATION
Wxwxwxwxwxw  2020 - Present
";
    // A known-REGULAR anchor: the identical 11-char string, rendered as a
    // plain paragraph (not an entry title), so its weight is never in
    // question. Under a section heading (not the leading preamble) so
    // `model::adapter::is_title_like` can't promote it into the header title
    // slot instead of a body paragraph.
    const REGULAR_REFERENCE: &str = "\
Jane Doe

SUMMARY
Wxwxwxwxwxw
";

    let t = template_style(TemplateId::SwissMinimal);
    let exp_svg = svg_page1(&model_from_resume_text(EXPERIENCE_ONLY), &t, false);
    let edu_svg = svg_page1(&model_from_resume_text(EDUCATION_ONLY), &t, false);
    let reg_svg = svg_page1(&model_from_resume_text(REGULAR_REFERENCE), &t, false);

    // The title (body-pt) and date (body-pt - 1pt) render at slightly
    // different baseline y-coordinates — different font sizes within the same
    // grid row do not share one text-layout baseline — so each is already its
    // OWN entry in `text_lines`' per-baseline grouping; `.pop()` (the
    // bottom-most line) isolates the title's own glyphs without the date's.
    let exp_title = text_lines(&exp_svg)
        .pop()
        .expect("experience fixture must render at least one text line");
    let edu_title = text_lines(&edu_svg)
        .pop()
        .expect("education fixture must render at least one text line");
    let reg_line = text_lines(&reg_svg)
        .pop()
        .expect("regular-reference fixture must render at least one text line");

    let exp_width = exp_title.2 - exp_title.1;
    let edu_width = edu_title.2 - edu_title.1;
    let reg_width = reg_line.2 - reg_line.1;

    assert!(
        (exp_width - edu_width).abs() < 0.5,
        "education entry title width ({edu_width:.2}pt) must match the \
         experience entry title width ({exp_width:.2}pt) for the identical \
         string \"Wxwxwxwxwxw\" — a mismatch means one rendered bold and the \
         other did not"
    );
    // Anchors the pair above to an absolute floor: a regression that
    // un-bolds BOTH entry titles would keep them equal to each other while
    // silently converging on the regular-weight width — this catches that
    // shape specifically, which the equality check alone cannot.
    assert!(
        exp_width > reg_width + 1.0,
        "experience entry title width ({exp_width:.2}pt) must be \
         measurably wider than the known-regular rendering of the same \
         string \"Wxwxwxwxwxw\" ({reg_width:.2}pt) — otherwise the title \
         is not actually bold"
    );
    assert!(
        edu_width > reg_width + 1.0,
        "education entry title width ({edu_width:.2}pt) must be \
         measurably wider than the known-regular rendering of the same \
         string \"Wxwxwxwxwxw\" ({reg_width:.2}pt) — otherwise the title \
         is not actually bold"
    );
}

/// #28 regression guard: the entry-title → subtitle baseline gap must be
/// non-trivial, not crammed against the title. Renders a single entry with a
/// subtitle and nothing after it, so the last two [`text_lines`] are (title,
/// subtitle) in that order; asserts the baseline distance between them clears
/// a per-template threshold picked ~1pt below what the SAME measurement gives
/// at the current `sp-subtitle-gap = 6pt` (measured directly, not estimated):
/// SwissMinimal 8.56pt (1pt) → 10.56pt (3pt) → 13.56pt (6pt), CologneNavy
/// 6.65pt (1pt) → 8.65pt (3pt) → 11.65pt (6pt). 3pt still read as crammed in
/// owner review across academic/aria/atelier/awesome/cadence and more, in
/// Experience, Projects AND Education — hence the second bump. The baseline
/// gap is mostly font leading (not spacing alone), which is why the two
/// templates' absolute numbers differ, and why each gets its own floor
/// instead of one shared constant. Covers both spacing families:
/// `single_column.typ` (SwissMinimal) and the independently implemented
/// `cologne_navy.typ`, which never shares code with it.
#[test]
fn entry_subtitle_gap_is_non_trivial_regression() {
    const FIXTURE: &str = "\
Jane Doe

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
";

    for (id, min_gap) in [
        (TemplateId::SwissMinimal, 12.5),
        (TemplateId::CologneNavy, 10.5),
    ] {
        let t = template_style(id);
        let svg = svg_page1(&model_from_resume_text(FIXTURE), &t, false);
        let lines = text_lines(&svg);
        assert!(
            lines.len() >= 2,
            "{id:?}: expected at least a title line and a subtitle line, got {} lines",
            lines.len()
        );
        let subtitle = lines[lines.len() - 1];
        let title = lines[lines.len() - 2];
        let gap = subtitle.0 - title.0;
        assert!(
            gap > min_gap,
            "{id:?}: title→subtitle baseline gap ({gap:.2}pt) must clear {min_gap:.2}pt \
             — a regression to a ~1pt `sp-subtitle-gap` measures ~2pt lower here"
        );
    }
}

/// #28 regression guard, the cheaper sibling of the render assertion above:
/// for the remaining templates touched by the same fix (mostly cover-letter
/// letterheads, where building a full `LetterModel` fixture per layout for a
/// pixel measurement would be disproportionate to the risk — none of them has
/// Awesome's band-height interaction), assert the embedded `.typ` source
/// itself references `sp-name-below` at its name→contact call site rather
/// than a hardcoded literal. Each embeds its OWN copy of the template file
/// (not `engine.rs`'s private `include_str!` consts — module-private, not
/// visible to this file), so this reads the exact bytes compiled into the
/// binary, not a stale copy. A per-file minimum count (existing legitimate
/// uses + the one this fix added) means re-hardcoding the contact gap back to
/// a numeric literal drops the count and fails here.
#[test]
fn letterhead_and_header_templates_reference_sp_name_below_not_a_literal() {
    const CASES: &[(&str, &str, usize)] = &[
        (
            "meridian.typ",
            include_str!("../templates/meridian.typ"),
            1, // the fixed band contact block (Meridian has no other use)
        ),
        (
            "lebenslauf.typ",
            include_str!("../templates/lebenslauf.typ"),
            2, // pre-existing name block + the fixed contact block
        ),
        (
            "letter.typ",
            include_str!("../templates/letter.typ"),
            2, // pre-existing "name only" branch + the fixed contact block
        ),
        (
            "letter_banded.typ",
            include_str!("../templates/letter_banded.typ"),
            1,
        ),
        (
            "letter_navy.typ",
            include_str!("../templates/letter_navy.typ"),
            1,
        ),
        (
            "letter_monogram.typ",
            include_str!("../templates/letter_monogram.typ"),
            1,
        ),
        (
            "letter_sidebar.typ",
            include_str!("../templates/letter_sidebar.typ"),
            1, // was already 9pt, now tokenized instead of a bare literal
        ),
    ];
    for (name, source, min_count) in CASES {
        let count = source.matches("sp-name-below").count();
        assert!(
            count >= *min_count,
            "{name}: expected >= {min_count} use(s) of `sp-name-below`, found {count} \
             — the name→contact gap must not be re-hardcoded to a numeric literal"
        );
    }
}
