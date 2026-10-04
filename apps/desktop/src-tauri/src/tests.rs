use super::is_native_host_launch;

/// Build the argv tail a browser would pass (owned `String`s, as
/// `std::env::args` yields).
fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn windows_firefox_manifest_path_is_native_host() {
    // The regression: the `.firefox` infix broke the old `ends_with` match.
    assert!(is_native_host_launch(args(&[
        r"C:\Users\me\AppData\...\app.aijobhunter.bridge.firefox.json",
        "job-importer@aijobhunter.app",
    ])));
}

#[test]
fn windows_chrome_manifest_path_is_native_host() {
    assert!(is_native_host_launch(args(&[
        r"C:\Users\me\...\app.aijobhunter.bridge.chrome.json",
    ])));
}

#[test]
fn unix_manifest_path_is_native_host() {
    assert!(is_native_host_launch(args(&[
        "/home/me/.mozilla/native-messaging-hosts/app.aijobhunter.bridge.json",
    ])));
}

#[test]
fn chrome_extension_origin_is_native_host() {
    assert!(is_native_host_launch(args(&[
        "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll/",
    ])));
}

#[test]
fn deep_link_launch_is_not_native_host() {
    assert!(!is_native_host_launch(args(&["ajh://settings/extension"])));
}

#[test]
fn empty_launch_is_not_native_host() {
    assert!(!is_native_host_launch(args(&[])));
}

#[test]
fn unrelated_json_arg_is_not_native_host() {
    // A `.json` arg that does NOT contain the host name must not match.
    assert!(!is_native_host_launch(args(&[
        "/tmp/some-other-config.json"
    ])));
}

#[test]
fn host_name_in_directory_is_not_native_host() {
    // The host name in a DIRECTORY component (not the filename) must NOT match —
    // only the manifest basename counts.
    assert!(!is_native_host_launch(args(&[
        "/opt/app.aijobhunter.bridge/other.json"
    ])));
}

// ── agent-CLI argv sentinel (issue #1084 PR 1) ───────────────────────────

use super::is_agent_cli_launch;

#[test]
fn agent_sentinel_as_first_arg_is_agent_cli() {
    assert!(is_agent_cli_launch(&args(&["agent", "best-matches"])));
    assert!(is_agent_cli_launch(&args(&["agent"])));
}

#[test]
fn agent_elsewhere_in_argv_is_not_the_sentinel() {
    // Only the FIRST post-exe token counts — a later `agent` (e.g. a job
    // verb's own url containing the word) must not trigger CLI mode.
    assert!(!is_agent_cli_launch(&args(&["job", "agent"])));
}

#[test]
fn empty_launch_is_not_agent_cli() {
    assert!(!is_agent_cli_launch(&args(&[])));
}

#[test]
fn deep_link_launch_is_not_agent_cli() {
    assert!(!is_agent_cli_launch(&args(&["ajh://settings/extension"])));
}

#[test]
fn native_host_argv_is_not_agent_cli() {
    // The two sentinels must never overlap on the same argv shape.
    assert!(!is_agent_cli_launch(&args(&[
        "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll/",
    ])));
}
