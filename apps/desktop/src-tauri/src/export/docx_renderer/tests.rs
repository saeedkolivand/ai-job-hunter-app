use super::*;

#[test]
fn pt_to_half_points_is_distinct_from_pt_to_dxa() {
    // The bug this guards: font size (`w:sz`, half-points) was routed through
    // the `dxa` (twentieths-of-a-point) conversion, producing a 10× oversize —
    // 10.5pt → 210 (105pt rendered) instead of 21 (10.5pt rendered).
    assert_eq!(pt_to_half_points(10.5), 21);
    assert_eq!(pt_to_dxa(10.5), 210);
    assert_ne!(pt_to_half_points(10.5), pt_to_dxa(10.5));
}

#[test]
fn pt_to_half_points_rounds_rather_than_truncates() {
    // Half-points make odd sizes representable (10.5pt → 21 exactly); a
    // truncating `as usize` would still be correct for whole points but wrong
    // for anything that lands on a `.25`/`.75` boundary once doubled.
    assert_eq!(pt_to_half_points(20.0), 40);
    assert_eq!(pt_to_half_points(9.0), 18);
    assert_eq!(pt_to_half_points(9.5), 19);
    assert_eq!(pt_to_half_points(6.25), 13); // 12.5 rounds up to 13
}

#[test]
fn mm_to_dxa_matches_word_a4_and_letter() {
    // Word writes A4 as 11906 × 16838 dxa and US Letter as 12240 × 15840.
    assert_eq!(mm_to_dxa(210.0), 11906);
    assert_eq!(mm_to_dxa(297.0), 16838);
    assert_eq!(mm_to_dxa(215.9), 12240);
    assert_eq!(mm_to_dxa(279.4), 15840);
}

#[test]
fn fallback_fonts_are_common_system_faces() {
    // Every bundled family resolves to a face present on Windows/Office so the
    // reader never has to silently substitute an un-embedded bundled font.
    assert_eq!(docx_fallback_font(FontFamily::Calibri), "Calibri");
    assert_eq!(docx_fallback_font(FontFamily::Inter), "Calibri");
    assert_eq!(docx_fallback_font(FontFamily::Manrope), "Calibri");
    assert_eq!(docx_fallback_font(FontFamily::SourceSerif4), "Georgia");
}
