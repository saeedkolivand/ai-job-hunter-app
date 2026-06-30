use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use zip::write::{ExtendedFileOptions, FileOptions};
use zip::{CompressionMethod, ZipWriter};

use crate::autopilot_helpers::redact_token;
use crate::error::{AppError, AppResult};

/// Redact every whitespace-delimited token in every line, preserving line
/// structure. Blank / whitespace-only lines become empty strings.
fn redact_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.split_whitespace()
                    .map(redact_token)
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build a redacted diagnostics zip at `dest`.
///
/// Strict allowlist — only `crashes.log`, `logs/<name>`, and a generated
/// `system-info.txt` are written. All other data-dir content is excluded by
/// construction (no wholesale dir walk). Text files are run through
/// [`redact_token`] per whitespace-delimited token before being zipped.
/// Missing inputs (no `crashes.log`, no `log_dir`) are non-fatal; the zip will
/// still be valid and contain at minimum `system-info.txt`.
///
/// `crashes.log` is read from `data_dir` (the panic-hook writes it there).
/// Log files produced by `tauri-plugin-log` are read from `log_dir`, which is
/// `app_log_dir()` — a **different** base directory from `app_data_dir()` on
/// Windows (`…\Local\…` vs `…\Roaming\…`) and macOS (`~/Library/Logs/…` vs
/// `~/Library/Application Support/…`). Passing them as separate parameters
/// prevents the two from ever being conflated.
///
/// `pub(crate)` so unit tests can call it without a Tauri harness.
pub(crate) fn build_diagnostics_zip(
    data_dir: &Path,
    log_dir: Option<&Path>,
    dest: &Path,
    app_version: &str,
) -> AppResult<()> {
    let file = std::fs::File::create(dest)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<ExtendedFileOptions> =
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    // ── system-info.txt — generated, already clean ────────────────────────
    {
        use sysinfo::System;
        let sys = System::new_all();
        let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_owned());
        let os_ver = System::os_version().unwrap_or_else(|| "unknown".to_owned());
        let arch = std::env::consts::ARCH;
        let total_ram_mb = sys.total_memory() / (1024 * 1024);
        let info = format!(
            "OS: {os_name} {os_ver}\nArch: {arch}\nApp version: {app_version}\nTotal RAM: {total_ram_mb} MB\n",
        );
        zip.start_file("system-info.txt", opts.clone())
            .map_err(|e| AppError::Storage(e.to_string()))?;
        zip.write_all(info.as_bytes())?;
    }

    // ── crashes.log (if present and is a plain file, not a symlink) — redacted ─
    // `symlink_metadata` does NOT follow symlinks: a symlink at crashes.log
    // reports `file_type().is_symlink()` rather than `is_file()`, so we skip it.
    // Defense-in-depth against a crafted symlink pointing at the SQLite store or
    // a résumé that would otherwise be read and included in the PUBLIC issue bundle.
    //
    // Bytes are read first and decoded with `from_utf8_lossy` so a single invalid
    // UTF-8 byte (e.g. from a corrupted crash) produces a replacement character
    // instead of aborting the entire export.
    let crashes_path = data_dir.join("crashes.log");
    if crashes_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_file())
    {
        let raw_bytes = std::fs::read(&crashes_path)?;
        let raw = String::from_utf8_lossy(&raw_bytes);
        zip.start_file("crashes.log", opts.clone())
            .map_err(|e| AppError::Storage(e.to_string()))?;
        zip.write_all(redact_lines(&raw).as_bytes())?;
    }

    // ── plugin log files — each redacted, with flat entry name logs/<name> ──
    // `tauri-plugin-log` with `TargetKind::LogDir { file_name: None }` writes
    // files directly into `app_log_dir()` (e.g. `ajh-tauri.log` plus rotated
    // `ajh-tauri_<date>.log` siblings). That is `log_dir` here — NOT a
    // subdirectory of `data_dir`. Reads are best-effort: an unreadable file or
    // a directory-listing failure skips that file without aborting the bundle.
    if let Some(log_dir) = log_dir {
        if log_dir.is_dir() {
            if let Ok(rd) = std::fs::read_dir(log_dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    // Skip non-files and symlinks — same defense-in-depth as crashes.log.
                    let Ok(meta) = path.symlink_metadata() else {
                        continue;
                    };
                    if !meta.file_type().is_file() {
                        continue;
                    }
                    let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let raw_bytes = match std::fs::read(&path) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let raw = String::from_utf8_lossy(&raw_bytes);
                    let entry_name = format!("logs/{fname}");
                    zip.start_file(&entry_name, opts.clone())
                        .map_err(|e| AppError::Storage(e.to_string()))?;
                    zip.write_all(redact_lines(&raw).as_bytes())?;
                }
            }
        }
    }

    zip.finish().map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(())
}

/// Build a redacted diagnostics zip at the caller-supplied `dest` path and
/// return `{ "success": true, "path": dest }` on success.
///
/// The renderer is responsible for obtaining `dest` via the save-file dialog
/// (tauri-plugin-dialog) and for revealing the file afterward
/// (tauri-plugin-opener). The zip contains exactly:
///   - `system-info.txt`  — generated OS/arch/version info, no user data
///   - `crashes.log`      — if present, every token redacted
///   - `logs/<name>`      — for each file in `app_log_dir()`, every token redacted
///
/// SQLite stores, documents, embeddings, credentials, and all other data-dir
/// content are excluded by construction.
#[tauri::command]
pub async fn support_export_diagnostics(app: AppHandle, dest: String) -> Value {
    let data_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    // `app_log_dir()` uses a different base directory from `app_data_dir()` on
    // Windows (Local vs Roaming) and macOS (Library/Logs vs Library/Application
    // Support). If the path resolver fails, skip logs gracefully rather than
    // aborting the whole bundle.
    let log_dir = app.path().app_log_dir().ok();
    let app_version = env!("CARGO_PKG_VERSION");
    let dest_path = std::path::PathBuf::from(&dest);
    // Offload sync file I/O to the blocking pool.
    match tokio::task::spawn_blocking(move || {
        build_diagnostics_zip(&data_dir, log_dir.as_deref(), &dest_path, app_version)
    })
    .await
    {
        Ok(Ok(())) => json!({ "success": true, "path": dest }),
        Ok(Err(e)) => json!({ "success": false, "error": e }),
        Err(e) => json!({ "success": false, "error": format!("task panicked: {e}") }),
    }
}

#[tauri::command]
pub async fn support_get_system_info(_app: AppHandle) -> Value {
    // Stub - implement when needed
    json!(null)
}

#[cfg(test)]
mod tests {
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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, Some(lp), &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, Some(lp), &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

        assert!(names.contains("crashes.log"), "crashes.log missing");
        assert!(names.contains("logs/app.log"), "logs/app.log missing");
        assert!(names.contains("system-info.txt"), "system-info.txt missing");
        assert_eq!(names.len(), 3, "unexpected entries: {names:?}");
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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, None, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
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

    /// When both crashes.log and log_dir are absent the zip is still valid and
    /// contains only system-info.txt.
    #[test]
    fn missing_optional_inputs_produce_valid_zip_with_system_info() {
        let data = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");

        build_diagnostics_zip(data.path(), None, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names = zip_entry_names(&bytes);
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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        // Before the fix this returned Err (read_to_string fails on invalid UTF-8).
        // Now it must succeed with lossy decoding.
        build_diagnostics_zip(dp, None, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();
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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(data.path(), Some(lp), &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();
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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, None, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

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

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(data.path(), Some(lp), &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

        assert!(
            names.contains("logs/ajh-tauri.log"),
            "real log file must be included; entries: {names:?}"
        );
        assert!(
            !names.contains("logs/secret.log"),
            "symlinked log must be skipped; entries: {names:?}"
        );
    }
}
