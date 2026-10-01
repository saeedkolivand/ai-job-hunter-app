//! Unit tests for the svg_geometry measurement helpers themselves.

use super::svg_geometry::{glyph_positions, svg_translation};

//
// Every geometry assertion in this file is only as trustworthy as these three
// functions. Their old failure mode was SILENT: an unparsed transform became
// `(0.0, 0.0)` and a self-closing `<g/>` unbalanced the stack, both of which
// move measurements without failing anything. These pin the hardened behavior.

#[test]
fn svg_translation_parses_the_forms_typst_actually_emits() {
    assert_eq!(svg_translation("translate(3 4)"), (3.0, 4.0));
    assert_eq!(svg_translation("translate(3,4)"), (3.0, 4.0));
    // One-argument form: ty is 0, not "unparsed".
    assert_eq!(svg_translation("translate(5)"), (5.0, 0.0));
    // The baseline y-flip.
    assert_eq!(svg_translation("matrix(1 0 0 -1 7 8)"), (7.0, 8.0));
    assert_eq!(svg_translation("matrix(1 0 0 1 7 8)"), (7.0, 8.0));
    assert_eq!(svg_translation("translate(-2.5 -0.75)"), (-2.5, -0.75));
}

#[test]
#[should_panic(expected = "unsupported transform")]
fn svg_translation_rejects_scale_instead_of_reading_it_as_zero() {
    // Used to return (0.0, 0.0): a scaled group's contents would be reported at
    // their parent's origin, and every containment assertion would still pass.
    svg_translation("scale(2)");
}

#[test]
#[should_panic(expected = "unsupported transform")]
fn svg_translation_rejects_rotate_instead_of_reading_it_as_zero() {
    svg_translation("rotate(90)");
}

#[test]
#[should_panic(expected = "non-identity, non-y-flip linear part")]
fn svg_translation_rejects_a_scaling_matrix() {
    // Shape-wise a valid 6-number matrix, so the old `starts_with("matrix(")`
    // arm accepted it and returned (e, f) — silently dropping a 2x scale of
    // every child coordinate.
    svg_translation("matrix(2 0 0 2 10 20)");
}

#[test]
fn glyph_positions_ignores_self_closing_groups() {
    // `xmlwriter` writes `<g …/>` for a group it never put children into. It has
    // no `</g>`, so pushing it left the stack permanently deep and shifted every
    // later sibling — here, the second glyph would read x=1030 instead of 30.
    let svg = concat!(
        r##"<svg xmlns="http://www.w3.org/2000/svg">"##,
        r##"<g transform="translate(10 20)"><use x="5" fill="#111111"/></g>"##,
        r##"<g transform="translate(1000 1000)"/>"##,
        r##"<g transform="translate(30 40)"><use x="0" fill="#222222"/></g>"##,
        r##"</svg>"##,
    );
    let glyphs = glyph_positions(svg);
    assert_eq!(
        glyphs,
        vec![
            (15.0, 20.0, "#111111".to_string()),
            (30.0, 40.0, "#222222".to_string()),
        ],
        "a self-closing <g/> must contribute no offset to its siblings"
    );
}

#[test]
fn glyph_positions_accumulates_nested_group_translations() {
    // The property every measurement here relies on, pinned directly.
    let svg = concat!(
        r##"<svg xmlns="http://www.w3.org/2000/svg">"##,
        r##"<g transform="translate(10 20)">"##,
        r##"<g transform="matrix(1 0 0 -1 1 2)"><use x="3" fill="#abcdef"/></g>"##,
        r##"</g>"##,
        r##"<use x="7" fill="#000000"/>"##,
        r##"</svg>"##,
    );
    assert_eq!(
        glyph_positions(svg),
        vec![
            (14.0, 22.0, "#abcdef".to_string()),
            // After `</g></g>` the stack is back at the root, not still nested.
            (7.0, 0.0, "#000000".to_string()),
        ]
    );
}

#[test]
#[should_panic(expected = "underflowed")]
fn walk_svg_tags_rejects_an_unbalanced_closing_group() {
    glyph_positions(r#"<svg><g transform="translate(1 1)"></g></g></svg>"#);
}

#[test]
#[should_panic(expected = "unclosed")]
fn walk_svg_tags_rejects_a_truncated_document() {
    glyph_positions(r##"<svg><g transform="translate(1 1)"><use x="0" fill="#fff"/>"##);
}
