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
/// Missing inputs (no `crashes.log`, no `logs/`) are non-fatal; the zip will
/// still be valid and contain at minimum `system-info.txt`.
///
/// `pub(crate)` so unit tests can call it without a Tauri harness.
pub(crate) fn build_diagnostics_zip(
    data_dir: &Path,
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
    let crashes_path = data_dir.join("crashes.log");
    if crashes_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_file())
    {
        let raw = std::fs::read_to_string(&crashes_path)?;
        zip.start_file("crashes.log", opts.clone())
            .map_err(|e| AppError::Storage(e.to_string()))?;
        zip.write_all(redact_lines(&raw).as_bytes())?;
    }

    // ── logs/ — each file redacted, with flat entry name logs/<name> ─────
    let logs_dir = data_dir.join("logs");
    if logs_dir.is_dir() {
        let rd = std::fs::read_dir(&logs_dir)?;
        for entry in rd.flatten() {
            let path = entry.path();
            // Skip non-files and symlinks — same defense-in-depth as crashes.log.
            // `symlink_metadata` does not follow symlinks, so a symlink inside logs/
            // pointing at sensitive data is skipped rather than read and zipped.
            let Ok(meta) = path.symlink_metadata() else {
                continue;
            };
            if !meta.file_type().is_file() {
                continue;
            }
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let raw = std::fs::read_to_string(&path)?;
            let entry_name = format!("logs/{fname}");
            zip.start_file(&entry_name, opts.clone())
                .map_err(|e| AppError::Storage(e.to_string()))?;
            zip.write_all(redact_lines(&raw).as_bytes())?;
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
///   - `logs/<name>`      — for each file in `logs/`, every token redacted
///
/// SQLite stores, documents, embeddings, credentials, and all other data-dir
/// content are excluded by construction.
#[tauri::command]
pub async fn support_export_diagnostics(app: AppHandle, dest: String) -> Value {
    let data_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let app_version = env!("CARGO_PKG_VERSION");
    let dest_path = std::path::PathBuf::from(&dest);
    // Offload sync file I/O to the blocking pool.
    match tokio::task::spawn_blocking(move || {
        build_diagnostics_zip(&data_dir, &dest_path, app_version)
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

    /// The zip must contain exactly crashes.log + logs/app.log + system-info.txt
    /// and must NEVER include the SQLite DB or document file.
    #[test]
    fn allowlist_excludes_sensitive_files() {
        let data = TempDir::new().unwrap();
        let dp = data.path();

        // Allowed inputs
        std::fs::write(dp.join("crashes.log"), "panic: something happened").unwrap();
        std::fs::create_dir(dp.join("logs")).unwrap();
        std::fs::write(dp.join("logs").join("app.log"), "WARN something").unwrap();

        // Sensitive — must never appear in the zip
        std::fs::write(dp.join("store.db"), b"SQLite data").unwrap();
        std::fs::create_dir(dp.join("documents")).unwrap();
        std::fs::write(dp.join("documents").join("resume.pdf"), b"%PDF").unwrap();

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, &dest, "0.0.0-test").unwrap();

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
        build_diagnostics_zip(dp, &dest, "0.0.0-test").unwrap();

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
        assert!(!content.contains("alice"), "username must not leak; got: {content}");
        assert!(
            !content.contains("supersecret"),
            "credential value must not leak; got: {content}"
        );
    }

    /// When both crashes.log and logs/ are absent the zip is still valid and
    /// contains only system-info.txt.
    #[test]
    fn missing_optional_inputs_produce_valid_zip_with_system_info() {
        let data = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");

        build_diagnostics_zip(data.path(), &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names = zip_entry_names(&bytes);
        assert_eq!(names, vec!["system-info.txt"]);
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
        build_diagnostics_zip(dp, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

        assert!(
            !names.contains("crashes.log"),
            "symlinked crashes.log must be skipped; entries: {names:?}"
        );
        // Only the always-generated entry survives.
        assert!(names.contains("system-info.txt"));
    }

    /// A symlink inside logs/ pointing at a sensitive file must be skipped; other
    /// real log files in the same directory must still be included.
    #[cfg(unix)]
    #[test]
    fn symlinked_log_file_inside_logs_dir_is_skipped() {
        use std::os::unix::fs as unix_fs;

        let data = TempDir::new().unwrap();
        let dp = data.path();

        std::fs::create_dir(dp.join("logs")).unwrap();
        // A real log file — must be included.
        std::fs::write(dp.join("logs").join("app.log"), "INFO startup").unwrap();

        // Sensitive target outside the data dir.
        let secret_dir = TempDir::new().unwrap();
        std::fs::write(secret_dir.path().join("credentials.db"), b"creds").unwrap();

        // Symlink logs/secret.log → sensitive file — must be skipped.
        unix_fs::symlink(
            secret_dir.path().join("credentials.db"),
            dp.join("logs").join("secret.log"),
        )
        .unwrap();

        let dest_dir = TempDir::new().unwrap();
        let dest = dest_dir.path().join("diag.zip");
        build_diagnostics_zip(dp, &dest, "0.0.0-test").unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let names: HashSet<String> = zip_entry_names(&bytes).into_iter().collect();

        assert!(
            names.contains("logs/app.log"),
            "real log file must be included; entries: {names:?}"
        );
        assert!(
            !names.contains("logs/secret.log"),
            "symlinked log must be skipped; entries: {names:?}"
        );
    }
}
