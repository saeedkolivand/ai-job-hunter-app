use super::*;

fn argv(s: &str) -> Vec<String> {
    // Mimic a real launch: exe path first, then the URL the OS appends.
    vec!["C:/app/ajh.exe".to_string(), s.to_string()]
}

#[test]
fn accepts_a_valid_autopilot_url() {
    assert_eq!(
        parse_focus_target(&argv("ajh://autopilot/job-1a2b3c")),
        Some(FocusTarget::Autopilot("job-1a2b3c".to_string()))
    );
    // uuid-shaped id
    assert_eq!(
        parse_focus_target(&argv(
            "ajh://autopilot/550e8400-e29b-41d4-a716-446655440000"
        )),
        Some(FocusTarget::Autopilot(
            "550e8400-e29b-41d4-a716-446655440000".to_string()
        ))
    );
}

#[test]
fn rejects_unknown_scheme_or_action() {
    assert_eq!(parse_focus_target(&argv("https://autopilot/x")), None);
    assert_eq!(parse_focus_target(&argv("ajhx://autopilot/x")), None);
    assert_eq!(parse_focus_target(&argv("AJH://autopilot/x")), None); // case-sensitive
    assert_eq!(parse_focus_target(&argv("ajh://settings/wipe")), None);
    assert_eq!(parse_focus_target(&argv("ajh://privacy/reset")), None);
}

#[test]
fn rejects_path_traversal_and_injection_shapes() {
    assert_eq!(
        parse_focus_target(&argv("ajh://autopilot/../../settings")),
        None
    );
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a/b")), None); // extra segment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/")), None); // empty id
    assert_eq!(parse_focus_target(&argv("ajh://autopilot")), None); // no id segment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/id?x=1")), None); // query
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/id#frag")), None); // fragment
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a b")), None); // space
    assert_eq!(parse_focus_target(&argv("ajh://autopilot/a\\b")), None); // backslash
    assert_eq!(
        parse_focus_target(&argv(&format!("ajh://autopilot/{}", "x".repeat(65)))),
        None
    );
}

#[test]
fn ignores_normal_launch_argv() {
    assert_eq!(parse_focus_target(&["C:/app/ajh.exe".to_string()]), None);
    assert_eq!(parse_focus_target(&[]), None);
}

#[test]
fn finds_the_url_among_other_args() {
    let v = vec![
        "C:/app/ajh.exe".to_string(),
        "--flag".to_string(),
        "ajh://autopilot/abc".to_string(),
    ];
    assert_eq!(
        parse_focus_target(&v),
        Some(FocusTarget::Autopilot("abc".to_string()))
    );
}
