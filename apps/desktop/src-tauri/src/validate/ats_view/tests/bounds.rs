//! Hostile input stays cheap and bounded: line, text, section and role caps.

use super::super::{ats_view_from_text, AtsHints, AtsSectionKind, AtsStatus, AtsView};

fn view(text: &str) -> AtsView {
    ats_view_from_text(text, AtsHints::default())
}

fn timed(text: &str) -> (AtsView, std::time::Duration) {
    let start = std::time::Instant::now();
    let v = view(text);
    (v, start.elapsed())
}

/// End to end on hostile single lines (the line cap and the heading window together);
/// `the_heading_window_alone_keeps_the_splitter_linear` isolates the window.
// Ceiling is generous for loaded CI debug builds; a quadratic regression exceeds it by orders of magnitude.
#[test]
fn one_huge_line_of_bullet_glyphs_is_fast() {
    for line in [
        "▪".repeat(200_000),
        "■ X".repeat(66_000),
        "●A ".repeat(70_000),
    ] {
        let (v, took) = timed(&line);
        assert!(took.as_secs_f32() < 10.0, "took {took:?}");
        assert!(v.sections.len() <= 64 && v.roles.len() <= 64);
    }
}

#[test]
fn many_headings_and_roles_are_capped() {
    let mut text = String::from("Jane Doe\nWORK EXPERIENCE\n");
    for i in 0..500 {
        text.push_str(&format!("Acme {i} Corp 2020 - 2021\nEngineer\n"));
    }
    for _ in 0..500 {
        text.push_str("SKILLS\n");
    }
    let (v, took) = timed(&text);
    assert!(took.as_secs_f32() < 10.0, "took {took:?}");
    assert_eq!(v.roles.len(), 64);
    // 64 detected headings, THEN the Missing core row (Education) — the cap never drops it.
    assert_eq!(v.sections.len(), 65);
    assert!(v
        .sections
        .iter()
        .any(|s| s.kind == AtsSectionKind::Education && s.status == AtsStatus::Missing));
    assert_eq!(v.summary.total as usize, 4 + 1 + 65 + 64 * 4);
}

#[test]
fn a_real_heading_at_the_start_of_a_very_long_line_still_splits() {
    // ~120K characters: far past the line cap, so this also proves the cap does not eat it.
    let v = view(&format!("Jane Doe\nSKILLS• {}\n", "Rust, ".repeat(20_000)));
    assert!(v
        .sections
        .iter()
        .any(|s| s.heading.as_deref() == Some("SKILLS")));
}

/// Calls the splitter directly with a line LONGER than the line cap, so the
/// `MAX_HEADING_BYTES` window (not `split_lines`' cap) is what keeps it linear.
#[test]
fn the_heading_window_alone_keeps_the_splitter_linear() {
    use super::super::text::split_fused_headings;
    for line in [
        "▪".repeat(500_000),
        "■ X".repeat(200_000),
        format!("{}•x", "A".repeat(500_000)),
    ] {
        let start = std::time::Instant::now();
        let out = split_fused_headings(vec![line]);
        let took = start.elapsed();
        assert!(took.as_secs_f32() < 10.0, "took {took:?}");
        assert_eq!(
            out.len(),
            1,
            "no known heading in the window: nothing is split"
        );
    }
    let out = split_fused_headings(vec![format!("SKILLS{}", "▪".repeat(500_000))]);
    assert_eq!(out[0], "SKILLS");
    assert_eq!(out.len(), 2);
}
