use super::*;
// ── mcp --help / launch-arg parsing (items 10, 11, 23, 28) ──────────────

#[test]
fn parse_launch_args_accepts_any_subset_of_the_two_flags_in_any_order() {
    assert_eq!(parse_launch_args(&[]).unwrap(), LaunchArgs::default());
    assert_eq!(
        parse_launch_args(&args(&["--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: false,
            http: None,
        }
    );
    assert_eq!(
        parse_launch_args(&args(&["--allow-irreversible", "--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: true,
            http: None,
        },
        "order must not matter"
    );
}

#[test]
fn parse_launch_args_http_takes_a_bare_port_and_nothing_else() {
    assert_eq!(
        parse_launch_args(&args(&["--http", "8090"])).unwrap().http,
        Some(8090)
    );
    assert_eq!(
        parse_launch_args(&args(&["--allow-irreversible", "--http", "8090"]))
            .unwrap()
            .http,
        Some(8090)
    );
    // No flag-shape lets a caller name a host or address (issue #1173's "refuse at parse
    // time"): a bare port is the only thing `--http` ever accepts.
    assert!(parse_launch_args(&args(&["--http", "0.0.0.0:9000"])).is_err());
    assert!(parse_launch_args(&args(&["--http=9000"])).is_err());
    assert!(parse_launch_args(&args(&["--http", "not-a-port"])).is_err());
    assert!(parse_launch_args(&args(&["--http"])).is_err());
    assert!(parse_launch_args(&args(&["--http", "-1"])).is_err());
    assert!(parse_launch_args(&args(&["--http", "99999"])).is_err());
}

#[test]
fn parse_launch_args_accepts_help_anywhere_and_rejects_anything_else() {
    assert!(parse_launch_args(&args(&["--help"])).unwrap().help);
    assert!(
        parse_launch_args(&args(&["--allow-reversible", "--help"]))
            .unwrap()
            .help
    );
    assert!(parse_launch_args(&args(&["not-a-flag"])).is_err());
    assert!(parse_launch_args(&args(&["--allow-reversible", "typo"])).is_err());
}

#[test]
fn mcp_help_text_lists_both_flags_and_derives_its_default_list_from_tools() {
    let text = mcp_help_text();
    assert!(text.contains("--allow-reversible"));
    assert!(text.contains("--allow-irreversible"));
    for name in [
        "best-matches",
        "job",
        "profile",
        "automations",
        "commands",
        "call-read",
    ] {
        assert!(text.contains(name), "missing default tool `{name}`: {text}");
    }
    let default_line = text
        .lines()
        .find(|l| l.starts_with("Default"))
        .expect("must have a 'Default (no flags): ...' line");
    assert!(
        !default_line.contains("call-reversible") && !default_line.contains("call-irreversible"),
        "the default-tool-list line must not name a gated tool: {default_line}"
    );
}

#[test]
fn print_help_never_adds_a_trailing_blank_line() {
    let mut buf = Vec::new();
    print_help(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "must end in exactly one newline, no extra blank line: {text:?}"
    );
}
