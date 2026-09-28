use super::*;
use tempfile::TempDir;

// ── Mutation-check note ──────────────────────────────────────────────
// The two `#[cfg(unix)]` tests below were mutation-checked in a
// standalone repro against the PRE-FIX shape (`std::fs::write` then a
// best-effort `set_permissions` afterward, `let _ =`-swallowed): BOTH
// still PASS against that old code. Its final on-disk state — including
// the pre-existing-file self-heal — is identical to the fix's, because
// the old code's `set_permissions` call, though its failure was
// discarded, still ran and still succeeded on every ordinary run. A
// concurrent-poller race test (permissive umask, a 2,000,000-iteration
// busy-poll thread racing the write, 4 independent runs) also never
// observed the old code's create-then-chmod window even once — it is
// real (two separate syscalls) but far too short (no I/O wait between
// them) for a black-box test to reliably land inside. So: these tests
// verify END-STATE correctness (and guard a future refactor that drops
// the trailing `set_permissions` call entirely, which WOULD fail them)
// — they do NOT, and structurally cannot, prove the TOCTOU window is
// closed. That property is undetectable by test; it is verified by
// reading `persist_token`'s doc comment above (the mode is now part of
// the `open(2)` call itself, not a later chmod).

#[test]
fn persist_token_writes_readable_content() {
    let dir = TempDir::new().unwrap();
    persist_token(dir.path(), "abc123").unwrap();
    let read = std::fs::read_to_string(dir.path().join(TOKEN_FILE)).unwrap();
    assert_eq!(read, "abc123");
}

#[cfg(unix)]
#[test]
fn persist_token_creates_file_owner_only_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    persist_token(dir.path(), "abc123").unwrap();
    let mode = std::fs::metadata(dir.path().join(TOKEN_FILE))
        .unwrap()
        .permissions()
        .mode();
    // Mask off the file-type bits `st_mode` packs alongside the
    // permission bits — only the permission bits are under test.
    assert_eq!(
        mode & 0o777,
        0o600,
        "pairing token file must be owner-only (0o600), got {:o}",
        mode & 0o777
    );
}

#[cfg(unix)]
#[test]
fn persist_token_corrects_a_preexisting_wrong_permission_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join(TOKEN_FILE);
    // Simulate a token file that predates this fix — or was created on a
    // filesystem/umask combination that widened it — group- and
    // world-readable.
    std::fs::write(&path, "stale").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    persist_token(dir.path(), "fresh").unwrap();

    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "a pre-existing wide-permission token file must self-heal on the next persist"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh");
}
