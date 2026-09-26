use super::*;

// ── pointer + token file reads (pure fs, no env mutation needed here —
// the path itself is exercised by platform::config's own tests) ────────

#[test]
fn read_pairing_token_trims_and_rejects_empty() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join(TOKEN_FILE), "  abc123  \n").unwrap();
    assert_eq!(
        read_pairing_token(dir.path().to_str().unwrap()),
        Some("abc123".to_string())
    );

    let empty_dir = tempfile::TempDir::new().unwrap();
    std::fs::write(empty_dir.path().join(TOKEN_FILE), "   \n").unwrap();
    assert_eq!(read_pairing_token(empty_dir.path().to_str().unwrap()), None);

    let missing_dir = tempfile::TempDir::new().unwrap();
    assert_eq!(
        read_pairing_token(missing_dir.path().to_str().unwrap()),
        None
    );
}

// ── UNC `dataDir` guard (MEDIUM fix — security review) ─────────────────

#[test]
fn rejects_unc_and_double_slash_data_dirs() {
    assert!(!is_safe_local_data_dir(r"\\attacker.example.com\share"));
    assert!(!is_safe_local_data_dir("//attacker.example.com/share"));
    // A mixed-separator UNC path is still UNC.
    assert!(!is_safe_local_data_dir(r"\\attacker.example.com/share"));
}

/// MEDIUM fix, security review round 2: Windows treats `/` and `\`
/// interchangeably, so a UNC root written with ONE separator of each
/// kind (`\/host\share`, `/\host/share`) is still an absolute UNC path —
/// confirmed against `ntpath`'s parser — and the pre-fix
/// `starts_with(r"\\") || starts_with("//")` check let both straight
/// through, since neither is a literal two-backslash or two-slash
/// prefix.
#[test]
fn rejects_a_unc_data_dir_with_mixed_leading_separators() {
    assert!(!is_safe_local_data_dir(r"\/attacker.example.com\share"));
    assert!(!is_safe_local_data_dir(r"/\attacker.example.com/share"));
}

#[test]
fn rejects_a_relative_data_dir() {
    assert!(!is_safe_local_data_dir("relative/path"));
    assert!(!is_safe_local_data_dir(""));
}

#[test]
fn accepts_a_normal_absolute_local_path() {
    let dir = tempfile::TempDir::new().unwrap();
    assert!(is_safe_local_data_dir(dir.path().to_str().unwrap()));
}

#[test]
fn read_pairing_token_never_touches_the_filesystem_for_a_unc_data_dir() {
    // No real UNC share exists in a hermetic test — the proof this guard
    // works is that it returns `None` WITHOUT ever attempting the read
    // (a real attempt against an unreachable host would hang/timeout,
    // not return promptly).
    assert_eq!(read_pairing_token(r"\\attacker.example.com\share"), None);
}
