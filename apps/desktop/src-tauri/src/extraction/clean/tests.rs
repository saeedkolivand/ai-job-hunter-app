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

#[test]
fn kerned_dates_are_rejoined_and_other_digit_runs_are_kept() {
    assert_eq!(
        rejoin_kerned_dates("Designer | Acme  0 2 /2 0 2 3  - 0 1 /2 0 2 5"),
        "Designer | Acme  02/2023  - 01/2025"
    );
    assert_eq!(rejoin_kerned_dates("2 0 1 9 - 2 0 2 1"), "2019 - 2021");
    // A kerned phone and an ordinary digit list are not dates.
    assert_eq!(
        rejoin_kerned_dates("+3 1 6 3 4 7 8 0 9 3 6"),
        "+3 1 6 3 4 7 8 0 9 3 6"
    );
    assert_eq!(rejoin_kerned_dates("steps 1 2 3"), "steps 1 2 3");
    assert_eq!(
        rejoin_kerned_dates("Exam scores: 2 0 2 3"),
        "Exam scores: 2 0 2 3"
    );
    assert_eq!(
        rejoin_kerned_dates("since 2 0 2 1 – now"),
        "since 2021 – now"
    );
    assert_eq!(
        rejoin_kerned_dates("02/2023 - 01/2025"),
        "02/2023 - 01/2025"
    );
}

#[test]
fn a_rejoined_entry_line_gets_its_date_parsed() {
    let kerned = "EXPERIENCE\n\nDesigner | Acme  0 2 /2 0 2 3  - 0 1 /2 0 2 5\n- Did things\n";
    let date_of = |text: &str| {
        crate::model::adapter::model_from_resume_text(text)
            .sections
            .iter()
            .flat_map(|s| &s.blocks)
            .find_map(|b| match b {
                crate::model::document::Block::Entry(e) => e.date.clone(),
                _ => None,
            })
    };
    assert_eq!(
        date_of(kerned),
        None,
        "the kerned form must be the failing case"
    );
    assert_eq!(
        date_of(&rejoin_kerned_dates(kerned)).as_deref(),
        Some("02/2023  - 01/2025")
    );
}
