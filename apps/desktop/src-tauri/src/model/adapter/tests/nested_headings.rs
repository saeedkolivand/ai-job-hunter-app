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

/// An H1 name is not a parent section, so `###` section headings under it stay
/// sections (pre-PR review of #1440).
#[test]
fn h3_sections_under_an_h1_name_stay_sections() {
    let m = model_from_resume_text(
        "# Jane Doe

### Experience
Business Analyst | Acme
2022 - Present

### Education
BBA | Example University
2018 - 2021
",
    );
    assert_eq!(
        section_ids(&m),
        vec![&SectionId::Experience, &SectionId::Education]
    );
}

/// A known section name stays a section even beneath a `##` heading.
#[test]
fn known_section_name_h3_under_h2_stays_a_section() {
    let m = model_from_resume_text(
        "## Summary
Analyst with five years in market research.

### Skills
Market research, forecasting.
",
    );
    assert!(
        section_ids(&m).contains(&&SectionId::Skills),
        "{:?}",
        section_headings(&m)
    );
}

/// A later `#` heading ends the `##` parent, so `###` below it is a section again.
#[test]
fn h1_after_h2_ends_the_parent_section() {
    let m = model_from_resume_text(
        "## Experience\n### Analyst | Acme\n2022 - Present\n\n# Jane Doe\n\n### Projects\nShipped things.\n",
    );
    assert!(
        section_headings(&m).iter().any(|h| h.contains("Projects")),
        "{:?}",
        section_headings(&m)
    );
}

/// An entry heading with no date line below it has no date.
#[test]
fn nested_entry_heading_without_date_line_has_no_date() {
    let m = model_from_resume_text("## Experience\n### Analyst | Acme\n- Built reports.\n");
    let exp = section_entries(find_section(&m, SectionId::Experience).expect("experience"));
    assert_eq!(exp.len(), 1);
    assert_eq!(exp[0].date, None);
}
