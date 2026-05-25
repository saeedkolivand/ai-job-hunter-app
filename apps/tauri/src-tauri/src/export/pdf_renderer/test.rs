use super::*;
use crate::export::templates::Template;

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
    let segments = vec![
        TextSegment { text: "Hello World".to_string(), bold: false },
    ];
    let lines = wrap_segments(&segments, 50.0, 12.0);
    assert!(!lines.is_empty());
}

#[test]
fn test_push_word_to_segs_same_bold() {
    let mut segs = vec![TextSegment { text: "Hello".to_string(), bold: false }];
    push_word_to_segs(&mut segs, "World", false, true);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "Hello World");
}

#[test]
fn test_push_word_to_segs_different_bold() {
    let mut segs = vec![TextSegment { text: "Hello".to_string(), bold: false }];
    push_word_to_segs(&mut segs, "World", true, true);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].text, " World");
    assert!(segs[1].bold);
}

#[test]
fn test_push_word_to_segs_punctuation() {
    let mut segs = vec![TextSegment { text: "Hello".to_string(), bold: false }];
    push_word_to_segs(&mut segs, ",", false, true);
    assert_eq!(segs[0].text, "Hello,");
}
