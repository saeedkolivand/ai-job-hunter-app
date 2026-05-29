use super::*;
use crate::export::templates::Template;
use crate::export::types::FontFamily;
use crate::measure::FontMetrics;

#[test]
fn test_rgb_to_color() {
    let color = rgb_to_color((255, 0, 0));
    if let Color::Rgb(rgb) = color {
        assert!((rgb.r - 1.0).abs() < 0.01);
        assert!((rgb.g - 0.0).abs() < 0.01);
        assert!((rgb.b - 0.0).abs() < 0.01);
    } else {
        panic!("Expected RGB color");
    }
}

#[test]
fn test_rgb_to_color_white() {
    let color = rgb_to_color((255, 255, 255));
    if let Color::Rgb(rgb) = color {
        assert!((rgb.r - 1.0).abs() < 0.01);
        assert!((rgb.g - 1.0).abs() < 0.01);
        assert!((rgb.b - 1.0).abs() < 0.01);
    } else {
        panic!("Expected RGB color");
    }
}

#[test]
fn test_pt_to_mm() {
    let mm = pt_to_mm(72.0);
    assert!((mm - 25.4).abs() < 0.1);
}

#[test]
fn test_setup_colors() {
    let template = Template::get(crate::export::types::TemplateId::Modern);
    let palette = setup_colors(&template);
    let _ = palette.name;
    let _ = palette.section;
}

#[test]
fn test_setup_layout() {
    let template = Template::get(crate::export::types::TemplateId::Modern);
    let layout = setup_layout(&template);
    assert_eq!(layout.page_width, 210.0);
    assert_eq!(layout.page_height, 297.0);
    assert!(layout.line_height > 0.0);
}

#[test]
fn test_build_text_ops_empty() {
    let config = TextOpsConfig {
        x: 10.0,
        y: 20.0,
        font_regular: FontId::new(),
        font_bold: FontId::new(),
        font_size: 12.0,
        normal_color: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
        bold_color: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
    };
    let ops = build_text_ops(&[], config);
    assert!(ops.is_empty());
}

#[test]
fn test_wrap_segments_empty() {
    let segments = vec![];
    let lines = wrap_segments(&segments, 100.0, 12.0);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].is_empty());
}

#[test]
fn test_wrap_segments_simple() {
    let segments = vec![TextSegment {
        text: "Hello World".to_string(),
        bold: false,
    }];
    let lines = wrap_segments(&segments, 50.0, 12.0);
    assert!(!lines.is_empty());
}

#[test]
fn test_push_word_to_segs_same_bold() {
    let mut segs = vec![TextSegment {
        text: "Hello".to_string(),
        bold: false,
    }];
    push_word_to_segs(&mut segs, "World", false, true);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "Hello World");
}

#[test]
fn test_push_word_to_segs_different_bold() {
    let mut segs = vec![TextSegment {
        text: "Hello".to_string(),
        bold: false,
    }];
    push_word_to_segs(&mut segs, "World", true, true);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].text, " World");
    assert!(segs[1].bold);
}

#[test]
fn test_push_word_to_segs_punctuation() {
    let mut segs = vec![TextSegment {
        text: "Hello".to_string(),
        bold: false,
    }];
    push_word_to_segs(&mut segs, ",", false, true);
    assert_eq!(segs[0].text, "Hello,");
}

// ─── Exact text geometry (replaces the × 0.52 / × 0.3 char-count hacks) ───────

#[test]
fn centered_x_centers_content() {
    // A 100mm-wide name on a 210mm page starts at (210-100)/2 = 55mm.
    assert!((centered_x(210.0, 100.0) - 55.0).abs() < 1e-4);
    // Content wider than the page yields a negative (off-page) start — the
    // caller's concern, not this helper's.
    assert!(centered_x(100.0, 120.0) < 0.0);
}

#[test]
fn text_advance_is_positive_and_grows_with_text() {
    let a = text_advance_mm("Jane Doe", FontFamily::Inter, true, 22.0);
    assert!(a > 0.0);
    let short = text_advance_mm("Hi", FontFamily::Inter, false, 11.0);
    let long = text_advance_mm("Hi there friend", FontFamily::Inter, false, 11.0);
    assert!(long > short, "more text should advance further");
}

#[test]
fn contact_link_rects_positions_links_by_real_advance() {
    let m = FontMetrics;
    let family = FontFamily::Inter;
    let size = 9.0;
    let x_start = 25.0;
    let text = "Berlin | [LinkedIn](https://linkedin.com/in/jane) | jane@example.com";

    let rects = contact_link_rects(text, x_start, family, size, &m);
    assert_eq!(rects.len(), 2, "one markdown link + one email link");

    // LinkedIn: left edge = x_start + advance("Berlin | "), width = advance("LinkedIn").
    let li = &rects[0];
    assert_eq!(li.url, "https://linkedin.com/in/jane");
    let expect_left = x_start + m.advance_mm("Berlin | ", family, false, size);
    assert!(
        (li.x_left_mm - expect_left).abs() < 0.01,
        "linkedin left {} vs expected {}",
        li.x_left_mm,
        expect_left
    );
    assert!((li.width_mm - m.advance_mm("LinkedIn", family, false, size)).abs() < 0.01);

    // Email becomes a mailto: link positioned after "Berlin | LinkedIn | ".
    let em = &rects[1];
    assert_eq!(em.url, "mailto:jane@example.com");
    let expect_left2 = x_start + m.advance_mm("Berlin | LinkedIn | ", family, false, size);
    assert!((em.x_left_mm - expect_left2).abs() < 0.01);
    assert!(em.x_left_mm > li.x_left_mm, "links advance left-to-right");
}

#[test]
fn contact_link_rects_empty_without_links() {
    let rects = contact_link_rects(
        "just plain contact text",
        20.0,
        FontFamily::Calibri,
        9.0,
        &FontMetrics,
    );
    assert!(rects.is_empty());
}

#[test]
fn contact_link_rects_differ_from_legacy_char_count_guess() {
    // The old code placed the rect at x_start + byte_count * (pt_to_mm(size) * 0.52),
    // i.e. proportional to character count. For a narrow prefix in a proportional
    // font the real advance is much smaller — assert the fix actually moved the rect.
    let m = FontMetrics;
    let family = FontFamily::Inter;
    let size = 9.0;
    let x_start = 20.0;
    let text = "iii | [GitHub](https://github.com/x)";

    let rects = contact_link_rects(text, x_start, family, size, &m);
    let legacy_char_w = pt_to_mm(size) * 0.52;
    let legacy_left = x_start + "iii | ".len() as f32 * legacy_char_w;
    assert!(
        (rects[0].x_left_mm - legacy_left).abs() > 0.1,
        "exact advance ({}) should differ from the char-count guess ({})",
        rects[0].x_left_mm,
        legacy_left
    );
}
