use super::*;

#[test]
fn removes_private_use_glyphs() {
    let input = "Email \u{f0e0} jane@example.com \u{f08c} LinkedIn";
    let out = strip_icon_glyphs(input);
    assert!(!out.contains('\u{f0e0}'));
    assert!(!out.contains('\u{f08c}'));
    assert!(out.contains("jane@example.com"));
    assert!(out.contains("LinkedIn"));
}

#[test]
fn keeps_newlines_and_tabs() {
    assert_eq!(strip_icon_glyphs("a\nb\tc"), "a\nb\tc");
}

#[test]
fn removes_replacement_char_and_controls() {
    assert_eq!(strip_icon_glyphs("a\u{FFFD}b\u{0007}c"), "abc");
}
