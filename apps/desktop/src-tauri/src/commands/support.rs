use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use zip::write::{ExtendedFileOptions, FileOptions};
use zip::{CompressionMethod, ZipWriter};

use crate::error::{AppError, AppResult};
use crate::observability::redact_tokens;

/// Redact every whitespace-delimited token in every line, preserving line
/// structure. Blank / whitespace-only lines become empty strings.
///
/// Shared with [`crate::crash_reporting`], which runs it over every outgoing
/// Sentry event. Both consumers are "text about to leave the machine", so they
/// must not drift apart into two redactors of differing strength (ADR-027) —
/// [`redact_tokens`] is the single implementation both funnel through.
pub(crate) fn redact_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                redact_tokens(line)
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
/// [`redact_tokens`] before being zipped.
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
    // ── Guard: reject dest if it aliases a bundle source ────────────────────
    // Use normalized-absolute paths (no symlink resolution — consistent with the
    // symlink-skip behavior below) so `./x` vs `x` can't sneak past the check.
    // `std::path::absolute` lexically resolves `.`/`..` without touching the
    // filesystem; we fall back to the raw path if the cwd can't be obtained.
    let crashes_path = data_dir.join("crashes.log");
    let dest_abs = std::path::absolute(dest).unwrap_or_else(|_| dest.to_path_buf());
    {
        let crashes_abs =
            std::path::absolute(&crashes_path).unwrap_or_else(|_| crashes_path.clone());
        if dest_abs == crashes_abs {
            return Err(AppError::Validation(
                "export destination must not overwrite a diagnostic source file".to_owned(),
            ));
        }
    }
    if let Some(ld) = log_dir {
        if ld.is_dir() {
            if let Ok(rd) = std::fs::read_dir(ld) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    let Ok(meta) = path.symlink_metadata() else {
                        continue;
                    };
                    if !meta.file_type().is_file() {
                        continue;
                    }
                    if std::path::absolute(&path).unwrap_or_else(|_| path.clone()) == dest_abs {
                        return Err(AppError::Validation(
                            "export destination must not overwrite a diagnostic source file"
                                .to_owned(),
                        ));
                    }
                }
            }
        }
    }
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
                    // Regression guard: skip dest itself even when it is a newly-created
                    // file inside log_dir (the pre-flight check only catches files that
                    // existed before File::create).
                    if path == dest_abs {
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
mod tests;
