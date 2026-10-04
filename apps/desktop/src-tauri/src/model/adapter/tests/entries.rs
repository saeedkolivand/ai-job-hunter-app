use super::{support::*, *};

/// Comma + parenthesized date format (AI documented output) yields
/// Block::Entry (bold title) not Block::Paragraph (non-bold text).
#[test]
fn comma_paren_date_yields_entry_block() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Senior Engineer, Acme Corp (January 2021 \u{2013} March 2023)
- Led a team of five engineers
- Shipped three major features
";
    let m = model_from_resume_text(resume);
    let experience =
        find_section(&m, SectionId::Experience).expect("experience section must be present");

    let entries = section_entries(experience);
    assert_eq!(
        entries.len(),
        1,
        "expected one Entry block for the job header"
    );

    let title_text = flat(&entries[0].title);
    assert!(
        title_text.contains("Senior Engineer"),
        "entry title must contain the role; got: {title_text:?}"
    );
    assert!(
        title_text.contains("Acme Corp"),
        "entry title must contain the company; got: {title_text:?}"
    );
    assert!(
        title_text.contains("January 2021"),
        "entry title must contain the date (whole line is bold); got: {title_text:?}"
    );
    assert!(
        entries[0].date.is_none(),
        "date must be None for comma+paren format (date is embedded in title); got: {:?}",
        entries[0].date
    );
    assert_eq!(
        entries[0].bullets.len(),
        2,
        "both bullets must attach to the entry"
    );
}

/// Pipe-separated with a date segment yields Block::Entry (bold title).
#[test]
fn pipe_date_segment_yields_entry_block() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Principal Engineer | Meridian Systems | 2019 \u{2013} Present
- Scaled the platform to 500 k events per second
";
    let m = model_from_resume_text(resume);
    let experience =
        find_section(&m, SectionId::Experience).expect("experience section must be present");

    let entries = section_entries(experience);
    assert_eq!(
        entries.len(),
        1,
        "expected one Entry block for the pipe-date line"
    );

    let title_text = flat(&entries[0].title);
    assert!(
        title_text.contains("Principal Engineer"),
        "entry title must contain role; got: {title_text:?}"
    );
    assert!(
        title_text.contains("Meridian Systems"),
        "entry title must contain company; got: {title_text:?}"
    );
    assert!(
        entries[0].date.is_none(),
        "date must be None for pipe-date format (date is embedded in title); got: {:?}",
        entries[0].date
    );
    assert_eq!(
        entries[0].bullets.len(),
        1,
        "bullet must attach to the entry"
    );
}

/// CRITICAL regression: `**bold**` inside a bullet must survive as a real
/// bold [`TextRun`](super::super::rich::TextRun), not just have its `**`
/// markers stripped. `BULLET_RE` used to capture the bullet's text from
/// the already-markdown-stripped `clean` string, so `raw`/`segments`
/// never saw the `**` in the first place — every bullet lost bold, in
/// every template, PDF and DOCX alike.
#[test]
fn bullet_with_bold_marker_produces_a_bold_text_run() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
- Migrated to **Rust** and cut latency
";
    let m = model_from_resume_text(resume);
    let experience = find_section(&m, SectionId::Experience).expect("experience section");
    let entry = section_entries(experience)
        .into_iter()
        .next()
        .expect("job entry");
    let bullet = &entry.bullets[0];
    assert!(
        bullet.iter().any(|r| r.bold && r.text == "Rust"),
        "expected a bold \"Rust\" run, got {bullet:?}"
    );
}

/// CRITICAL regression: a job-entry TITLE carrying `**bold**` must also
/// survive — the two-space-gap `JobEntry` arm computed its title from the
/// stripped `clean` string too, and the adapter tokenized `line.text`
/// (also stripped) rather than a markdown-preserving field.
#[test]
fn job_entry_title_with_bold_marker_produces_a_bold_text_run() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
**Acme Corp**  2020 - Present
- Led the platform team
";
    let m = model_from_resume_text(resume);
    let experience = find_section(&m, SectionId::Experience).expect("experience section");
    let entry = section_entries(experience)
        .into_iter()
        .next()
        .expect("job entry");
    assert!(
        entry.title.iter().any(|r| r.bold && r.text == "Acme Corp"),
        "expected a bold \"Acme Corp\" title run, got {:?}",
        entry.title
    );
    assert_eq!(entry.date.as_deref(), Some("2020 - Present"));
}

/// Owner-reported regression: the title/company line carries no date at
/// all (separated from the role by a middot, not a comma/pipe/paren), and
/// the date range + location sit on their OWN following line. Before the
/// next-line-date `JobEntry` branches, NEITHER line matched any recognized
/// job-entry shape, so the whole entry silently rendered as two unrelated
/// plain (non-bold) paragraphs instead of a structured, bold entry with a
/// distinguishable date and location subtitle.
#[test]
fn title_middot_company_then_bare_date_line_yields_structured_entry() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Senior Frontend Developer \u{b7} ACTINEO GmbH
December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland
- Built scalable applications
";
    let m = model_from_resume_text(resume);
    let experience = find_section(&m, SectionId::Experience).expect("experience section");

    let entries = section_entries(experience);
    assert_eq!(
        entries.len(),
        1,
        "expected exactly one structured entry, got blocks: {:?}",
        experience.blocks
    );
    let entry = entries[0];
    assert_eq!(
        flat(&entry.title),
        "Senior Frontend Developer \u{b7} ACTINEO GmbH"
    );
    assert_eq!(
        entry.date.as_deref(),
        Some("December 2022 \u{2013} November 2025")
    );
    assert_eq!(
        entry.subtitle.as_ref().map(flat).as_deref(),
        Some("K\u{f6}ln, Deutschland")
    );
    assert_eq!(entry.bullets.len(), 1);
    assert_eq!(flat(&entry.bullets[0]), "Built scalable applications");
}
