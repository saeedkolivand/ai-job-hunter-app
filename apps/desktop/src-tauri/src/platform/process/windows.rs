//! Windows binary resolution (`PATH` × `PATHEXT`, `.cmd`/`.bat` shims).
//!
//! npm-global CLIs (`gemini`, `codex`, `agy`, …) install on Windows as **`.cmd`
//! shims** (e.g. `…\npm\gemini.cmd`) with no `.exe`. `CreateProcess` — and thus a
//! bare `Command::new("gemini")` — only launches `.com`/`.exe`; it does not consult
//! `PATHEXT`, so the shim is reported "not found" for both detection and spawn.
//! [`resolve_cli_binary`] reproduces the shell's own lookup (search each `PATH`
//! dir for the name plus each `PATHEXT` extension) and reports whether the hit is
//! a batch shim that must be run through `cmd.exe`.

/// No CLI `PATH` augmentation on Windows — GUI apps there already inherit the
/// full `PATH`; see `unix::cli_path` for the macOS/Linux counterpart.
pub fn cli_path() -> Option<std::ffi::OsString> {
    None
}

/// No-op counterpart to `unix::reset_cli_path_cache` — Windows has nothing to
/// reset (`cli_path` above is not cached).
pub fn reset_cli_path_cache() {}

/// A CLI binary resolved on Windows: the concrete path found on `PATH`, and
/// whether it is a `.cmd`/`.bat` shim that must be launched through `cmd.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCli {
    /// Concrete resolved path, e.g. `…\npm\gemini.cmd` or `…\claude.exe`.
    pub path: std::path::PathBuf,
    /// `true` for a `.cmd`/`.bat` shim → launch via `cmd.exe /C <path> <args…>`.
    pub needs_cmd_wrapper: bool,
}

/// The Windows default when `PATHEXT` is unset.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WSF";

/// Resolve `binary` to a concrete path on Windows, searching `PATH` × `PATHEXT`
/// the way the shell does. `None` when nothing matches, so callers fall back to a
/// bare spawn and let the OS surface `NotFound`.
pub fn resolve_cli_binary(binary: &str) -> Option<ResolvedCli> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string());
    resolve_cli_binary_in(binary, path_var.as_os_str(), &pathext)
}

/// Pure core of [`resolve_cli_binary`] — `PATH`/`PATHEXT` are injected so it is
/// unit-testable with a fake directory and no process-env mutation. Mirrors
/// Windows' own lookup: bare name only if `binary` already has a `PATHEXT`
/// extension, else `binary + ext` per `PATHEXT` entry — never both.
fn resolve_cli_binary_in(
    binary: &str,
    path_var: &std::ffi::OsStr,
    pathext: &str,
) -> Option<ResolvedCli> {
    use std::path::Path;

    // An already-qualified path (a `<AGENT>_BIN` override like `C:\tools\gemini.cmd`,
    // or anything containing a separator) is honoured directly if it exists.
    let raw = Path::new(binary);
    if raw.is_absolute() || raw.components().count() > 1 {
        return raw.is_file().then(|| resolved(raw.to_path_buf()));
    }

    let pathext_entries: Vec<String> = pathext
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect();

    // A bare-name probe (no extension appended) only wins when `binary` already
    // carries a PATHEXT extension (e.g. `tool.cmd`); otherwise only `binary + ext`
    // is probed, per PATHEXT entry, so an extensionless shell shim (npm's
    // `#!/bin/sh` script next to `<name>.cmd`) never shadows its `.cmd` sibling.
    let binary_lower = binary.to_ascii_lowercase();
    let already_has_ext = pathext_entries
        .iter()
        .any(|ext| binary_lower.ends_with(&ext.to_ascii_lowercase()));
    let exts: Vec<String> = if already_has_ext {
        vec![String::new()]
    } else {
        pathext_entries
    };

    for dir in std::env::split_paths(path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{binary}{ext}"));
            if candidate.is_file() {
                return Some(resolved(candidate));
            }
        }
    }
    None
}

/// Tag a resolved path with whether it is a batch shim (`.cmd`/`.bat`).
fn resolved(path: std::path::PathBuf) -> ResolvedCli {
    let needs_cmd_wrapper = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);
    ResolvedCli {
        path,
        needs_cmd_wrapper,
    }
}

#[cfg(test)]
mod tests;
