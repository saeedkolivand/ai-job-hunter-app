use super::*;

// ── R5: Personio dotall regex — multi-line content capture ────────────────────
//
// `DESC_RE` uses the `(?s)` flag so `.` matches newlines.  Without it, a
// description spanning multiple lines would be truncated at the first `\n`.
// These tests are regression guards: they must FAIL if `(?s)` is removed.
//
// Same guard for `POSITION_RE`, which must capture multi-line position blocks.

#[test]
fn desc_re_captures_multiline_value_content() {
    let xml = r#"<jobDescription>
  <value>line one
line two
line three</value>
</jobDescription>"#;

    let cap = DESC_RE.captures(xml).expect("DESC_RE must match");
    let captured = cap.get(1).expect("group 1 must be present").as_str().trim();

    assert!(
        captured.contains("line one"),
        "DESC_RE must capture first line; got: {captured:?}"
    );
    assert!(
        captured.contains("line two"),
        "DESC_RE must capture second line (dotall required); got: {captured:?}"
    );
    assert!(
        captured.contains("line three"),
        "DESC_RE must capture third line (dotall required); got: {captured:?}"
    );
}

#[test]
fn position_re_captures_multiline_position_block() {
    let xml = r#"<position>
  <id>123</id>
  <name>Software Engineer</name>
  <jobDescription>
    <value>Build great things.
Work with great people.</value>
  </jobDescription>
</position>"#;

    let mut caps = POSITION_RE.captures_iter(xml);
    let cap = caps
        .next()
        .expect("POSITION_RE must match a position block");
    let block = cap.get(1).expect("group 1 must be present").as_str();

    assert!(
        block.contains("<id>123</id>"),
        "captured block must contain the id element; got: {block:?}"
    );
    assert!(
        block.contains("Software Engineer"),
        "captured block must span across newlines to include name; got: {block:?}"
    );
    assert!(
        block.contains("Build great things."),
        "captured block must include the description first line; got: {block:?}"
    );
}

#[test]
fn parse_xml_feed_extracts_multiline_description() {
    // End-to-end: parse_xml_feed must surface multiline descriptions intact
    // (after strip_html — here no HTML so the text passes through).
    let xml = r#"<?xml version="1.0"?>
<workzag-jobs>
  <position>
    <id>42</id>
    <name>Dev</name>
    <office>Berlin</office>
    <jobDescription>
      <value>Responsibility one.
Responsibility two.
Responsibility three.</value>
    </jobDescription>
    <createdAt>2024-01-01T00:00:00Z</createdAt>
  </position>
</workzag-jobs>"#;

    let positions = parse_xml_feed(xml);
    assert_eq!(positions.len(), 1, "one position must be parsed");
    let desc = &positions[0].description;
    assert!(
        desc.contains("Responsibility one."),
        "description must include first line; got: {desc:?}"
    );
    assert!(
        desc.contains("Responsibility two."),
        "description must include second line (dotall required); got: {desc:?}"
    );
    assert!(
        desc.contains("Responsibility three."),
        "description must include third line (dotall required); got: {desc:?}"
    );
}

#[test]
fn test_personio_scraper_id() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.id(), "personio");
}

#[test]
fn test_personio_scraper_display_name() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.display_name(), "Personio");
}

#[test]
fn test_personio_scraper_mode() {
    let scraper = PersonioScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_personio_hosts_not_empty() {
    assert!(!HOSTS.is_empty());
    assert!(HOSTS.contains(&"jobs.personio.de"));
    assert!(HOSTS.contains(&"jobs.personio.com"));
}

#[test]
fn test_personio_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}
