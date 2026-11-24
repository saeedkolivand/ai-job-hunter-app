//! Issue #1553 — an MCP `documents_export_document` reply carries the file as inline base64
//! (`data` + `dataEncoding`, ~70 KB per two-page CV in the agent's context, and a long document
//! trips [`super::MCP_RESULT_MAX_BYTES`]). This server runs on the user's machine next to the
//! agent, so it writes the bytes to a file and answers `path` + `bytes` instead. Done HERE, never
//! in the app's shared reshape: the plain `agent call` CLI and the renderer keep the base64 shape.
//!
//! Every failure (not an export, not base64, disk error) leaves the payload untouched, so the
//! call still answers the pre-existing inline shape, subject to the same size cap.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::Verb;

const EXPORT_COMMAND: &str = "documents_export_document";
const EXPORT_DIR: &str = "ajh-exports";
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Rewrites a dispatched export payload in place to `{path, bytes, filename, mimeType, …}`.
pub(in crate::extension_bridge::agent_cli::mcp) fn export_to_file(
    verb: &Verb,
    payload: &mut Value,
) {
    if !matches!(verb, Verb::Call { command, .. } if command == EXPORT_COMMAND) {
        return;
    }
    let dir = std::env::temp_dir().join(EXPORT_DIR);
    write_export(&dir, payload);
}

/// Pure core of [`export_to_file`], taking the target directory.
pub(super) fn write_export(dir: &Path, payload: &mut Value) {
    let Some(data) = payload.get_mut("data").and_then(Value::as_object_mut) else {
        return;
    };
    if data.get("dataEncoding").and_then(Value::as_str) != Some("base64") {
        return;
    }
    let Some(bytes) = data
        .get("data")
        .and_then(Value::as_str)
        .and_then(decode_base64)
    else {
        return;
    };
    let mime = data.get("mimeType").and_then(Value::as_str).unwrap_or("");
    let name = safe_name(data.get("filename").and_then(Value::as_str), mime);
    let Some(path) = write_unique(dir, &name, &bytes) else {
        return;
    };
    data.remove("data");
    data.remove("dataEncoding");
    data.insert("path".into(), json!(path.to_string_lossy()));
    data.insert("bytes".into(), json!(bytes.len()));
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

/// Basename only, no separators or `..`, a conservative charset, never empty.
fn safe_name(filename: Option<&str>, mime: &str) -> String {
    let base = filename
        .and_then(|f| f.rsplit(['/', '\\']).next())
        .unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c| c == '.' || c == ' ');
    if cleaned.is_empty() {
        let ext = match mime {
            "application/pdf" => "pdf",
            "text/plain" => "txt",
            m if m.contains("wordprocessingml") => "docx",
            _ => "bin",
        };
        return format!("export.{ext}");
    }
    cleaned.chars().take(120).collect()
}

/// The export dir, created owner-only. On Linux `temp_dir()` is the shared `/tmp`, so a dir
/// another user pre-created (or a symlink to one) is refused: the file holds a résumé.
fn private_dir(dir: &Path) -> Option<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .ok()?;
        let meta = std::fs::symlink_metadata(dir).ok()?;
        (meta.is_dir() && meta.permissions().mode() & 0o077 == 0).then_some(())
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir).ok()
}

fn write_unique(dir: &Path, name: &str, bytes: &[u8]) -> Option<PathBuf> {
    use std::io::Write;
    private_dir(dir)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let token = format!("{nanos:x}{:x}", COUNTER.fetch_add(1, Ordering::Relaxed));
    let path = dir.join(format!("{token}-{name}"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).ok()?;
    file.write_all(bytes).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests;
