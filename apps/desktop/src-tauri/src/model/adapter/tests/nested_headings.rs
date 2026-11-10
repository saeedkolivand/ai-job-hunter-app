use super::{support::*, *};

/// #1440: `### Title | Org` under a `## Section` is an entry inside that
/// section, and the date-only line beneath it is the entry's date.
const RESUME: &str = "\
Jane Doe
jane@example.test | Turin, Italy

## Experience
### Business Analyst | Example Company
January 2022 - Present
- Prepared weekly market reports and coordinated project schedules.

## Education
### Bachelor of Business Administration | Example University
2018 - 2021

## Skills
Market research, project coordination, spreadsheet analysis.
";

#[test]
fn nested_entry_headings_keep_content_and_dates() {
    let m = model_from_resume_text(RESUME);
    assert_eq!(
        section_ids(&m),
        vec![
            &SectionId::Experience,
            &SectionId::Education,
            &SectionId::Skills
        ]
    );

    let edu = section_entries(find_section(&m, SectionId::Education).expect("education kept"));
    assert_eq!(edu.len(), 1);
    let title = flat(&edu[0].title);
    assert!(
        title.contains("Bachelor of Business Administration"),
        "{title:?}"
    );
    assert!(title.contains("Example University"), "{title:?}");
    assert_eq!(edu[0].date.as_deref(), Some("2018 - 2021"));

    let exp = section_entries(find_section(&m, SectionId::Experience).expect("experience kept"));
    assert_eq!(exp.len(), 1);
    assert!(flat(&exp[0].title).contains("Business Analyst"));
    assert_eq!(exp[0].date.as_deref(), Some("January 2022 - Present"));
    assert_eq!(exp[0].bullets.len(), 1);
}

/// A lone `###` with no parent heading is still a section heading.
#[test]
fn h3_without_parent_heading_stays_a_section() {
    let m = model_from_resume_text("Jane Doe\n\n### Notable Work\nShipped things.\n");
    assert_eq!(section_headings(&m), vec!["Notable Work"]);
}
