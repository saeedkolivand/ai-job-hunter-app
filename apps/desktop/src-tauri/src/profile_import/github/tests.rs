use super::*;

fn repo(name: &str, stars: u64, fork: bool) -> RawRepo {
    RawRepo {
        name: name.to_string(),
        description: None,
        html_url: format!("https://github.com/u/{name}"),
        language: None,
        topics: vec![],
        stargazers_count: stars,
        pushed_at: None,
        fork,
    }
}

// ── username extraction ───────────────────────────────────────────────────

#[test]
fn parse_username_extracts_the_user_from_names_and_profile_urls() {
    for (input, expected, why) in [
        ("torvalds", "torvalds", "bare name"),
        ("  octocat  ", "octocat", "trims whitespace"),
        ("https://github.com/torvalds", "torvalds", "https url"),
        // Only the first path segment (the user) is taken, not the repo.
        (
            "https://github.com/torvalds/linux",
            "torvalds",
            "url with extra path",
        ),
        ("github.com/octocat", "octocat", "scheme-less url"),
        (
            "https://github.com/octocat?tab=repositories",
            "octocat",
            "strips trailing query",
        ),
        (
            "https://github.com/TorValds",
            "TorValds",
            "url preserves username case",
        ),
        (
            "HTTPS://www.github.com/octocat",
            "octocat",
            "www and mixed-case scheme",
        ),
    ] {
        assert_eq!(parse_username(input).unwrap(), expected, "{why}: {input}");
    }
}

// ── rejection of invalid / hostile input (SSRF guard) ─────────────────────

#[test]
fn parse_username_rejects_invalid_and_hostile_input() {
    let long = "a".repeat(40);
    for (input, why) in [
        ("   ", "empty"),
        // `../foo` must never become a username — first segment is `..`.
        ("../foo", "path traversal"),
        // A non-github host is rejected outright — its path is never even read as a
        // candidate username, and the host is never forwarded to the HTTP client.
        ("https://evil.com", "foreign host url"),
        ("https://evil.com/torvalds", "foreign host url with a path"),
        // `http://169.254.169.254/` — cloud metadata. No path segment → rejected,
        // and even with one it would have to pass the username regex.
        ("http://169.254.169.254/", "metadata url"),
        ("-bad", "leading hyphen"),
        ("bad-", "trailing hyphen"),
        ("a--b", "double hyphen"),
        (long.as_str(), "over 39 chars"),
        ("foo_bar", "disallowed char: underscore"),
        ("foo.bar", "disallowed char: dot"),
        // "https://github.com" — host only, no slash-after-host path segment.
        ("https://github.com", "bare host, no path segment"),
        // "https://github.com/" — trailing slash yields an empty first segment.
        ("https://github.com/", "trailing slash, empty segment"),
        // "https://github.com/-bad" — valid URL segment but invalid GitHub username
        // (leading hyphen). Two-stage rejection: URL parse succeeds, username validate fails.
        (
            "https://github.com/-bad",
            "url segment is an invalid github username",
        ),
    ] {
        assert!(
            matches!(parse_username(input), Err(AppError::Validation(_))),
            "{why}: {input}"
        );
    }
}

#[test]
fn accepts_max_length_and_internal_hyphen() {
    assert_eq!(parse_username("a-b").unwrap(), "a-b");
    let max = "a".repeat(39);
    assert_eq!(parse_username(&max).unwrap(), max);
}

// ── URL construction ──────────────────────────────────────────────────────

#[test]
fn api_url_is_constructed_from_username() {
    assert_eq!(
        api_url("octocat"),
        "https://api.github.com/users/octocat/repos?per_page=100&sort=updated&type=owner"
    );
}

// ── filter + sort ─────────────────────────────────────────────────────────

#[test]
fn forks_are_dropped() {
    let raw = vec![repo("real", 5, false), repo("forked", 100, true)];
    let out = filter_and_rank(raw);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "real");
}

#[test]
fn sorts_by_stars_descending() {
    let raw = vec![
        repo("low", 1, false),
        repo("high", 99, false),
        repo("mid", 50, false),
    ];
    let out = filter_and_rank(raw);
    let names: Vec<&str> = out.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["high", "mid", "low"]);
}

#[test]
fn equal_stars_break_ties_by_name() {
    let raw = vec![repo("zeta", 10, false), repo("alpha", 10, false)];
    let out = filter_and_rank(raw);
    assert_eq!(out[0].name, "alpha");
    assert_eq!(out[1].name, "zeta");
}

#[test]
fn caps_at_30() {
    let raw: Vec<RawRepo> = (0..50)
        .map(|i| repo(&format!("repo{i:02}"), i, false))
        .collect();
    let out = filter_and_rank(raw);
    assert_eq!(out.len(), MAX_REPOS);
    // Highest-starred must survive the cap.
    assert_eq!(out[0].stars, 49);
}

#[test]
fn maps_stargazers_count_to_stars() {
    let out = filter_and_rank(vec![repo("r", 7, false)]);
    assert_eq!(out[0].stars, 7);
}

// ── HTTP status mapping (item 1: pure map_status helper) ─────────────────

#[test]
fn map_status_404_is_validation() {
    assert!(matches!(map_status(404), Some(AppError::Validation(_))));
    if let Some(AppError::Validation(msg)) = map_status(404) {
        assert!(
            msg.to_lowercase().contains("not found"),
            "expected 'not found' in {msg:?}"
        );
    }
}

#[test]
fn map_status_403_and_429_are_rate_limited() {
    for code in [403, 429] {
        assert!(
            matches!(map_status(code), Some(AppError::RateLimited(_))),
            "{code}"
        );
    }
}

#[test]
fn map_status_500_and_503_are_network() {
    for code in [500, 503] {
        assert!(
            matches!(map_status(code), Some(AppError::Network(_))),
            "{code}"
        );
    }
}

#[test]
fn map_status_2xx_is_none() {
    for code in [200, 201, 299] {
        assert!(map_status(code).is_none(), "{code}");
    }
}

// ── serde: Some case (item 3) ─────────────────────────────────────────────

#[test]
fn output_some_pushed_at_is_present_and_camel_cased() {
    let repo = GitHubRepo {
        name: "my-project".to_string(),
        description: Some("A great project".to_string()),
        html_url: "https://github.com/u/my-project".to_string(),
        language: None,
        topics: vec![],
        stars: 1,
        pushed_at: Some("2026-01-10T12:00:00Z".to_string()),
    };
    let v = serde_json::to_value(&repo).unwrap();
    let obj = v.as_object().unwrap();
    // `Some` description must appear (not omitted).
    assert_eq!(obj["description"], "A great project");
    // `Some` pushedAt serialized under camelCase key — a future rename to
    // `pushed_at` (snake) or `PushedAt` is caught by this assertion.
    assert_eq!(obj["pushedAt"], "2026-01-10T12:00:00Z");
    // No snake_case leakage.
    assert!(
        !obj.contains_key("pushed_at"),
        "snake_case key must not appear"
    );
}

// ── output serialization ──────────────────────────────────────────────────

#[test]
fn output_omits_none_fields_and_camel_cases() {
    // `None` Options are omitted (skip_serializing_if) so the TS contract is
    // `description?: string`, not `string | null`; `htmlUrl`/`pushedAt` are
    // camelCase; `stargazers_count` is exposed as `stars`.
    let repo = GitHubRepo {
        name: "linux".to_string(),
        description: None,
        html_url: "https://github.com/torvalds/linux".to_string(),
        language: Some("C".to_string()),
        topics: vec!["kernel".to_string()],
        stars: 42,
        pushed_at: None,
    };
    let v = serde_json::to_value(&repo).unwrap();
    let obj = v.as_object().unwrap();

    assert!(!obj.contains_key("description"), "None must be omitted");
    assert!(!obj.contains_key("pushedAt"), "None must be omitted");
    assert!(!obj.contains_key("pushed_at"), "no snake_case key");
    assert_eq!(obj["htmlUrl"], "https://github.com/torvalds/linux");
    assert_eq!(obj["language"], "C");
    assert_eq!(obj["stars"], 42);
    assert!(!obj.contains_key("stargazers_count"), "renamed to stars");
}
