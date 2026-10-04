use std::collections::HashSet;
use std::io::Read;

use tempfile::TempDir;
use zip::ZipArchive;

use super::*;

fn zip_entry_names(bytes: &[u8]) -> Vec<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).expect("valid zip");
    (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_owned())
        .collect()
}

/// Build a diagnostics zip for `data`/`logs` into a fresh temp dir and return
/// its bytes.
fn build_zip(data: &Path, logs: Option<&Path>) -> Vec<u8> {
    let dest_dir = TempDir::new().unwrap();
    let dest = dest_dir.path().join("diag.zip");
    build_diagnostics_zip(data, logs, &dest, "0.0.0-test").unwrap();
    std::fs::read(&dest).unwrap()
}

fn zip_names(bytes: &[u8]) -> HashSet<String> {
    zip_entry_names(bytes).into_iter().collect()
}

fn read_zip_entry(bytes: &[u8], name: &str) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).expect("valid zip");
    let mut entry = archive.by_name(name).expect("entry not found");
    let mut out = String::new();
    entry.read_to_string(&mut out).unwrap();
    out
}

/// `crashes.log` must come from `data_dir`; log files must come from the
/// SEPARATE `log_dir`; a sensitive file in `data_dir` is excluded even
/// when the two directories are distinct. This prevents the two paths from
/// ever being conflated again (the Linux-coincidence bug).
#[test]
fn crashes_from_data_dir_and_logs_from_log_dir_are_independent() {
    let data = TempDir::new().unwrap();
    let dp = data.path();
    let logs = TempDir::new().unwrap();
    let lp = logs.path();

    // crashes.log in data_dir — must appear in zip
    std::fs::write(dp.join("crashes.log"), "panic: something happened").unwrap();
    // Sensitive file in data_dir — must NOT appear
    std::fs::write(dp.join("store.db"), b"SQLite data").unwrap();
    // Log file in log_dir (completely separate from data_dir) — must appear
    std::fs::write(lp.join("ajh-tauri.log"), "WARN something").unwrap();

    let names = zip_names(&build_zip(dp, Some(lp)));

    assert!(names.contains("crashes.log"), "crashes.log must be present");
    assert!(
        names.contains("logs/ajh-tauri.log"),
        "log from log_dir must appear under logs/; entries: {names:?}"
    );
    assert!(!names.contains("store.db"), "store.db must be excluded");
    assert_eq!(names.len(), 3, "unexpected entries: {names:?}");
}

/// The zip must contain exactly crashes.log + logs/app.log + system-info.txt
/// and must NEVER include the SQLite DB or document file.
#[test]
fn allowlist_excludes_sensitive_files() {
    let data = TempDir::new().unwrap();
    let dp = data.path();
    let log_dir = TempDir::new().unwrap();
    let lp = log_dir.path();

    // Allowed inputs
    std::fs::write(dp.join("crashes.log"), "panic: something happened").unwrap();
    std::fs::write(lp.join("app.log"), "WARN something").unwrap();

    // Sensitive — must never appear in the zip
    std::fs::write(dp.join("store.db"), b"SQLite data").unwrap();
    std::fs::create_dir(dp.join("documents")).unwrap();
    std::fs::write(dp.join("documents").join("resume.pdf"), b"%PDF").unwrap();

    let names = zip_names(&build_zip(dp, Some(lp)));

    assert!(names.contains("crashes.log"), "crashes.log missing");
    assert!(names.contains("logs/app.log"), "logs/app.log missing");
    assert!(names.contains("system-info.txt"), "system-info.txt missing");
    assert_eq!(names.len(), 3, "unexpected entries: {names:?}");
}

/// Redaction must not destroy the backtrace it is redacting.
///
/// A reported bundle's `crashes.log` had two PANIC entries whose backtrace
/// was completely empty, making both undiagnosable. Redaction is one of the
/// two candidates for that (the other being capture itself), so it gets
/// pinned here: symbol names and frame numbers survive, only the paths go.
#[test]
fn a_backtrace_survives_redaction_with_only_its_paths_removed() {
    let frame = "   3: ajh_tauri::window::move_state\n                                  at C:\\Users\\alice\\src\\window.rs:42:9";
    let redacted = redact_lines(frame);

    assert!(
        redacted.contains("ajh_tauri::window::move_state"),
        "the SYMBOL is the diagnostic value and must survive; got: {redacted}"
    );
    assert!(
        redacted.contains("3:"),
        "frame numbering must survive; got: {redacted}"
    );
    assert!(
        redacted.contains("<path-redacted>"),
        "the absolute path must still be redacted; got: {redacted}"
    );
    assert!(
        !redacted.contains("alice"),
        "username must not leak: {redacted}"
    );
}

/// The entry separator must survive too — without it, consecutive crashes
/// run together and the reader cannot tell where one backtrace ends.
#[test]
fn the_crash_entry_separator_survives_redaction() {
    assert_eq!(redact_lines("---"), "---");
}

/// Absolute paths and credential tokens in crashes.log must be redacted.
#[test]
fn crashes_log_paths_are_redacted() {
    let data = TempDir::new().unwrap();
    let dp = data.path();

    std::fs::write(
        dp.join("crashes.log"),
        "error at C:\\Users\\alice\\project\\foo.rs token=supersecret",
    )
    .unwrap();

    let bytes = build_zip(dp, None);
    let content = read_zip_entry(&bytes, "crashes.log");

    assert!(
        content.contains("<path-redacted>"),
        "absolute path must be redacted; got: {content}"
    );
    assert!(
        content.contains("<credential-redacted>"),
        "token= must be redacted; got: {content}"
    );
    assert!(
        !content.contains("alice"),
        "username must not leak; got: {content}"
    );
    assert!(
        !content.contains("supersecret"),
        "credential value must not leak; got: {content}"
    );
}

/// The support bundle is a SEPARATE redaction path from `sanitize_reason`
/// (board_health/autopilot) — ADR-027 requires the two never drift in
/// strength. A crash log echoing back an `Authorization: Bearer <token>`
/// header, or a bare GitHub/AWS-shaped key with no marker at all, must be
/// caught here too, not just in the other path.
#[test]
fn redact_lines_catches_bearer_tokens_and_bare_prefixed_secrets() {
    let redacted = redact_lines(
            "panic: Authorization: Bearer ghp_deadbeef12345 rejected\nleaked AKIAIOSFODNN7EXAMPLE in body",
        );
    assert!(
        !redacted.contains("ghp_deadbeef12345") && !redacted.contains("AKIAIOSFODNN7EXAMPLE"),
        "credential leaked through the support-bundle path; got: {redacted}"
    );
    assert!(
        redacted.matches("<credential-redacted>").count() >= 2,
        "both secrets must be redacted; got: {redacted}"
    );
}

/// When both crashes.log and log_dir are absent the zip is still valid and
/// contains only system-info.txt.
#[test]
fn missing_optional_inputs_produce_valid_zip_with_system_info() {
    let data = TempDir::new().unwrap();

    let names = zip_entry_names(&build_zip(data.path(), None));
    assert_eq!(names, vec!["system-info.txt"]);
}

/// A crashes.log containing an invalid UTF-8 byte must not abort the export.
/// The bundle is produced with a replacement character instead of erroring.
#[test]
fn non_utf8_in_crashes_log_does_not_abort_export() {
    let data = TempDir::new().unwrap();
    let dp = data.path();

    let mut bad: Vec<u8> = b"panic at boot ".to_vec();
    bad.push(0xFF); // lone byte — invalid UTF-8
    bad.extend_from_slice(b" more text");
    std::fs::write(dp.join("crashes.log"), &bad).unwrap();

    // Before the fix this returned Err (read_to_string fails on invalid UTF-8).
    // Now it must succeed with lossy decoding.
    let names = zip_names(&build_zip(dp, None));
    assert!(
        names.contains("crashes.log"),
        "crashes.log must still be included after lossy decode"
    );
}

/// A log file containing an invalid UTF-8 byte must not abort the export.
#[test]
fn non_utf8_in_log_file_does_not_abort_export() {
    let data = TempDir::new().unwrap();
    let logs = TempDir::new().unwrap();
    let lp = logs.path();

    let mut bad: Vec<u8> = b"WARN startup ".to_vec();
    bad.push(0xFE); // invalid UTF-8 byte
    std::fs::write(lp.join("ajh-tauri.log"), &bad).unwrap();

    let names = zip_names(&build_zip(data.path(), Some(lp)));
    assert!(
        names.contains("logs/ajh-tauri.log"),
        "log file must still be included after lossy decode"
    );
}

// ── M1: symlink skip (Unix only — Windows symlinks require elevated privileges) ──

/// A symlink at crashes.log pointing at a sensitive file must be silently
/// skipped rather than read and included in the bundle.
#[cfg(unix)]
#[test]
fn symlinked_crashes_log_is_skipped() {
    use std::os::unix::fs as unix_fs;

    let data = TempDir::new().unwrap();
    let dp = data.path();

    // Sensitive target outside the data dir (simulates the SQLite store).
    let secret_dir = TempDir::new().unwrap();
    std::fs::write(secret_dir.path().join("store.db"), b"SQLite sensitive").unwrap();

    // Symlink crashes.log → sensitive file.
    unix_fs::symlink(secret_dir.path().join("store.db"), dp.join("crashes.log")).unwrap();

    let names = zip_names(&build_zip(dp, None));

    assert!(
        !names.contains("crashes.log"),
        "symlinked crashes.log must be skipped; entries: {names:?}"
    );
    // Only the always-generated entry survives.
    assert!(names.contains("system-info.txt"));
}

/// A symlink inside log_dir pointing at a sensitive file must be skipped; other
/// real log files in the same directory must still be included.
#[cfg(unix)]
#[test]
fn symlinked_log_file_inside_log_dir_is_skipped() {
    use std::os::unix::fs as unix_fs;

    let data = TempDir::new().unwrap();
    let logs = TempDir::new().unwrap();
    let lp = logs.path();

    // A real log file — must be included.
    std::fs::write(lp.join("ajh-tauri.log"), "INFO startup").unwrap();

    // Sensitive target outside the log dir.
    let secret_dir = TempDir::new().unwrap();
    std::fs::write(secret_dir.path().join("credentials.db"), b"creds").unwrap();

    // Symlink log_dir/secret.log → sensitive file — must be skipped.
    unix_fs::symlink(
        secret_dir.path().join("credentials.db"),
        lp.join("secret.log"),
    )
    .unwrap();

    let names = zip_names(&build_zip(data.path(), Some(lp)));

    assert!(
        names.contains("logs/ajh-tauri.log"),
        "real log file must be included; entries: {names:?}"
    );
    assert!(
        !names.contains("logs/secret.log"),
        "symlinked log must be skipped; entries: {names:?}"
    );
}

/// `dest` pointing at `crashes.log` must be rejected before `File::create`
/// can truncate the source — the file must survive intact.
#[test]
fn dest_aliasing_crashes_log_is_rejected_without_truncating() {
    let data = TempDir::new().unwrap();
    let dp = data.path();
    let original = "crash: null pointer at boot";
    std::fs::write(dp.join("crashes.log"), original).unwrap();

    let dest = dp.join("crashes.log");
    let result = build_diagnostics_zip(dp, None, &dest, "0.0.0-test");

    assert!(
        result.is_err(),
        "should error when dest aliases crashes.log"
    );
    assert!(
        matches!(result.unwrap_err(), AppError::Validation(_)),
        "error must be Validation variant"
    );
    // Source file must not be truncated by the aborted File::create.
    assert_eq!(
        std::fs::read_to_string(dp.join("crashes.log")).unwrap(),
        original,
        "crashes.log must not be truncated"
    );
}

/// `dest` placed as a new path inside `log_dir` must not appear as an entry
/// in the produced zip — the regression guard in the log scan must skip it.
#[test]
fn dest_inside_log_dir_is_not_embedded_in_zip() {
    let data = TempDir::new().unwrap();
    let dp = data.path();
    let logs = TempDir::new().unwrap();
    let lp = logs.path();

    // A real log file that should be bundled.
    std::fs::write(lp.join("ajh-tauri.log"), "INFO boot").unwrap();

    // dest is a new path inside log_dir — the pre-flight check won't catch it
    // (no existing file at that path); the regression guard in the scan loop
    // must exclude it from the bundle even after File::create creates it.
    let dest = lp.join("diag.zip");
    build_diagnostics_zip(dp, Some(lp), &dest, "0.0.0-test").unwrap();

    let bytes = std::fs::read(&dest).unwrap();
    let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

    assert!(
        !names.contains("logs/diag.zip"),
        "dest must not be embedded in the zip; entries: {names:?}"
    );
    assert!(
        names.contains("logs/ajh-tauri.log"),
        "other log files must still be included; entries: {names:?}"
    );
}
