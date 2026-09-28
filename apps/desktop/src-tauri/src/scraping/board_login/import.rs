//! Import an existing job-board session straight from the user's installed
//! browser, so they can skip the in-app re-login flow.
//!
//! ## What it produces
//! The SAME artifacts the HTTP scrapers already consume — nothing downstream
//! changes:
//! * `<app_data_dir>/browser-state/<board_id>/cookies.json`  (`Vec<StoredCookie>`)
//! * `<app_data_dir>/browser-state/<board_id>/auth-status.json` (`connected = true`)
//!
//! Both are written through the existing `super::write_cookies` /
//! `super::write_auth_status` helpers, so the on-disk format is byte-identical
//! to a normal browser-login export.
//!
//! ## Dependency decision — why we hand-roll instead of using `rookie`
//! `rookie` would have given us per-domain Chromium extraction with DPAPI /
//! Keychain / libsecret decryption and a custom-db-path argument out of the box.
//! It is **unusable here**: `rookie 0.5.6` pins `rusqlite ^0.31`
//! (`libsqlite3-sys 0.28`), which collides with our `rusqlite 0.40`
//! (`libsqlite3-sys 0.38`) on the `links = "sqlite3"` native library — Cargo
//! refuses to link two copies. There is no feature toggle that reconciles the
//! pins. So we hand-roll the well-documented `v10`/`v11` path with `aes-gcm`,
//! recover the os_crypt key per-OS (DPAPI on Windows; "Safe Storage" password
//! from Keychain/libsecret + PBKDF2 on Unix), and read the cookies DB with the
//! `rusqlite` we already ship.
//!
//! ## Scope / limitations
//! * `v20` (App-Bound Encryption, Chrome 127+) is **not** decrypted here — the
//!   key is sealed to the browser process via an elevation service. If a board's
//!   cookies are all `v20` and none decrypt, we return [`ImportOutcome::Undecryptable`]
//!   rather than failing. (LinkedIn `li_at` is still `v10` in practice, so import
//!   keeps working today.)
//! * Windows has full `v10` parity. macOS/Linux are best-effort: the Safe-Storage
//!   password lookup + AES-128-CBC path is implemented but depends on the OS
//!   secret store being unlocked and readable.
//! * Cookie **values are never logged.** Only counts and outcomes are.

mod decrypt;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use super::{get_config, write_auth_status, write_cookies, StoredCookie};
use crate::observability::sanitize_reason;
use crate::platform::{detect_chromium_user_data_roots, ChromiumBrowser};

/// Result of an import attempt for a single board. Serializable so the command
/// layer can forward it to the renderer without a second mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ImportOutcome {
    /// `n` session cookies for the board were imported and the board's required
    /// marker (if any) was captured. `connected = true` was written.
    Imported(usize),
    /// Cookies for the board's domain were found in some browser but the
    /// required auth marker (e.g. LinkedIn `li_at`) was missing — the user is
    /// not actually logged in there.
    NoSession,
    /// The board's cookies exist but are all sealed with App-Bound Encryption
    /// (`v20`) or otherwise could not be decrypted on this machine.
    Undecryptable,
    /// No supported Chromium browser (Chrome/Edge/Brave) was found on disk.
    BrowserNotFound,
}

/// Import job-board session cookies for `board_id` from the user's installed
/// Chromium browsers and persist them as the scraper-consumed artifacts.
///
/// Best-effort by contract: a missing browser, a locked/absent profile, or a
/// decrypt failure each map to the appropriate non-error [`ImportOutcome`].
/// `Err` is reserved for genuinely unexpected IO (e.g. the destination dir
/// cannot be created).
pub fn import_cookies(app_data_dir: &Path, board_id: &str) -> Result<ImportOutcome> {
    let Some(config) = get_config(board_id) else {
        // Unknown board id — treat as "nothing to import" rather than an error
        // so a bad id from the UI never surfaces as a crash.
        return Ok(ImportOutcome::NoSession);
    };

    let roots = detect_chromium_user_data_roots();
    if roots.is_empty() {
        return Ok(ImportOutcome::BrowserNotFound);
    }

    let mut collected: Vec<StoredCookie> = Vec::new();
    let mut saw_undecryptable = false; // at least one v20/decrypt-fail row

    for (browser, root) in roots {
        match collect_from_root(browser, &root, board_id) {
            Ok(found) => {
                if found.saw_undecryptable {
                    saw_undecryptable = true;
                }
                collected.extend(found.cookies);
            }
            Err(e) => {
                // Per-browser failures are non-fatal: a locked DB we couldn't
                // copy, a missing Local State key, etc. Log without values.
                log::debug!(
                    "[cookie-import] {} root unreadable for {board_id}: {}",
                    browser.label(),
                    sanitize_reason(&e.to_string())
                );
            }
        }
    }

    // De-duplicate by (name, domain, path); later browsers win.
    collected.sort_by(|a, b| {
        (a.name.as_str(), a.domain.as_str(), a.path.as_str()).cmp(&(
            b.name.as_str(),
            b.domain.as_str(),
            b.path.as_str(),
        ))
    });
    collected.dedup_by(|a, b| a.name == b.name && a.domain == b.domain && a.path == b.path);

    if collected.is_empty() {
        // Nothing usable. Distinguish "all sealed" from "genuinely no session".
        return Ok(if saw_undecryptable {
            ImportOutcome::Undecryptable
        } else {
            ImportOutcome::NoSession
        });
    }

    // Decide "connected" using the board's own predicate where it has one
    // (LinkedIn → li_at). Boards without a predicate (indeed/xing/glassdoor) are
    // connected if we imported any session cookie for their domain.
    let connected = match config.is_authed_cookies {
        Some(predicate) => predicate(&collected),
        None => true,
    };

    if !connected {
        // We pulled the domain's cookies but the auth marker is absent → the
        // user isn't logged in there. Do NOT write a false "connected" status.
        return Ok(ImportOutcome::NoSession);
    }

    write_cookies(app_data_dir, board_id, &collected)?;
    write_auth_status(app_data_dir, board_id, true);

    Ok(ImportOutcome::Imported(collected.len()))
}

// ── Per-root collection ───────────────────────────────────────────────────────

struct RootHarvest {
    cookies: Vec<StoredCookie>,
    saw_undecryptable: bool,
}

/// Walk every profile under a single browser user-data root, reading the
/// board's cookies from each profile's `Network/Cookies` DB.
fn collect_from_root(browser: ChromiumBrowser, root: &Path, board_id: &str) -> Result<RootHarvest> {
    let key = decrypt::recover_os_crypt_key(root); // None → can't decrypt v10 in this root

    let mut harvest = RootHarvest {
        cookies: Vec::new(),
        saw_undecryptable: false,
    };

    for profile in profile_dirs(root) {
        let cookies_db = profile.join("Network").join("Cookies");
        if !cookies_db.exists() {
            continue;
        }
        match decrypt::read_profile_cookies(&cookies_db, board_id, key.as_deref()) {
            Ok(rows) => {
                if rows.saw_undecryptable {
                    harvest.saw_undecryptable = true;
                }
                harvest.cookies.extend(rows.cookies);
            }
            Err(e) => log::debug!(
                "[cookie-import] {} profile read failed for {board_id}: {}",
                browser.label(),
                sanitize_reason(&e.to_string())
            ),
        }
    }

    Ok(harvest)
}

/// Enumerate profile directories within a user-data root: `Default` plus every
/// `Profile N`. We list the root and keep dirs whose names match, rather than
/// probing a fixed set, so unusual profile numbers are covered.
fn profile_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let default = root.join("Default");
    if default.is_dir() {
        out.push(default);
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            // Defense-in-depth: a planted symlink under the browser root could
            // redirect the read elsewhere. Skip symlinked entries entirely.
            if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Profile ") && entry.path().is_dir() {
                out.push(entry.path());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
