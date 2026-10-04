use super::*;

// ── Changelog parsing ────────────────────────────────────────────────────────

/// `major.minor.patch` as a tuple for order comparisons in tests only — not a
/// general semver parser (a pre-release suffix like `-beta.1` just truncates at
/// the first non-numeric segment, which is fine for this repo's plain versions).
fn version_tuple(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split(['.', '-']).filter_map(|p| p.parse::<u32>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[test]
fn test_parse_heading_with_date() {
    assert_eq!(
        parse_heading("## [1.2.3](url) (2026-01-01)\n"),
        Some(("1.2.3".to_string(), Some("2026-01-01".to_string())))
    );
}

#[test]
fn test_parse_heading_without_date() {
    assert_eq!(
        parse_heading("## [1.2.3](url)\n"),
        Some(("1.2.3".to_string(), None))
    );
}

#[test]
fn test_parse_heading_ignores_subsections_and_title() {
    assert_eq!(parse_heading("### ✨ Features\n"), None);
    assert_eq!(parse_heading("# Changelog\n"), None);
    assert_eq!(parse_heading("just prose\n"), None);
}

#[test]
fn test_parse_changelog_two_versions() {
    let raw = "# Changelog\n\n\
        ## [2.0.0](url) (2026-02-02)\n\n### Features\n\n* second\n\n\
        ## [1.0.0](url) (2026-01-01)\n\n### Features\n\n* first\n";
    let entries = parse_changelog(raw);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].version, "2.0.0");
    assert_eq!(entries[0].date.as_deref(), Some("2026-02-02"));
    assert!(entries[0].body.contains("second"));
    assert!(!entries[0].body.contains("first"));
    assert_eq!(entries[1].version, "1.0.0");
    assert!(entries[1].body.contains("first"));
}

#[test]
fn test_parse_changelog_malformed_yields_no_entries() {
    assert!(parse_changelog("not a changelog at all\njust some prose").is_empty());
}

#[test]
fn test_parse_changelog_empty_string() {
    assert!(parse_changelog("").is_empty());
}

#[test]
fn test_changelog_response_malformed_never_panics() {
    let result = changelog_response("# Changelog\n\nno version headings here\n");
    assert_eq!(
        result["error"].as_str(),
        Some("Changelog unavailable (bundled CHANGELOG.md has no releases).")
    );
}

#[test]
fn test_changelog_response_empty_never_panics() {
    let result = changelog_response("");
    assert!(result["error"].is_string());
}

/// Parses the real, bundled `CHANGELOG.md` — guards against a future format
/// change in the generator (or in this parser) silently emptying the changelog.
#[test]
fn test_parse_changelog_real_bundled_file() {
    let entries = parse_changelog(CHANGELOG_MD);
    assert!(
        !entries.is_empty(),
        "bundled CHANGELOG.md should yield at least one release"
    );
    assert!(
        entries[0].date.is_some(),
        "newest release should have a date"
    );
    for pair in entries.windows(2) {
        assert!(
            version_tuple(&pair[0].version) >= version_tuple(&pair[1].version),
            "expected releases newest-first, got {} before {}",
            pair[0].version,
            pair[1].version
        );
    }
}

#[test]
fn test_changelog_response_real_bundled_file() {
    let result = changelog_response(CHANGELOG_MD);
    let releases = result["releases"]
        .as_array()
        .expect("bundled changelog should produce releases");
    assert!(!releases.is_empty());
    assert!(releases.len() <= CHANGELOG_LIMIT);
    let first_version = releases[0]["version"].as_str().unwrap();
    assert!(first_version.chars().next().unwrap().is_ascii_digit());
    assert!(releases[0]["url"]
        .as_str()
        .unwrap()
        .contains(&format!("releases/tag/v{first_version}")));
}
