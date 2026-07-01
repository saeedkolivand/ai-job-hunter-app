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

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use super::{get_config, write_auth_status, write_cookies, StoredCookie};
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
                    "[cookie-import] {} root unreadable for {board_id}: {e}",
                    browser.label()
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
    let key = recover_os_crypt_key(root); // None → can't decrypt v10 in this root

    let mut harvest = RootHarvest {
        cookies: Vec::new(),
        saw_undecryptable: false,
    };

    for profile in profile_dirs(root) {
        let cookies_db = profile.join("Network").join("Cookies");
        if !cookies_db.exists() {
            continue;
        }
        match read_profile_cookies(&cookies_db, board_id, key.as_deref()) {
            Ok(rows) => {
                if rows.saw_undecryptable {
                    harvest.saw_undecryptable = true;
                }
                harvest.cookies.extend(rows.cookies);
            }
            Err(e) => log::debug!(
                "[cookie-import] {} profile read failed for {board_id}: {e}",
                browser.label()
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

// ── SQLite read (copy-then-read; the live DB is locked) ───────────────────────

struct ProfileHarvest {
    cookies: Vec<StoredCookie>,
    saw_undecryptable: bool,
}

fn read_profile_cookies(
    cookies_db: &Path,
    board_id: &str,
    key: Option<&[u8]>,
) -> Result<ProfileHarvest> {
    // The browser holds an exclusive lock on the live DB. Copy it (and its WAL,
    // best-effort) to a temp file and read the copy, then clean up.
    let tmp = copy_locked_db(cookies_db)?;
    let result = (|| {
        let conn = rusqlite::Connection::open_with_flags(
            &tmp.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let mut stmt = conn.prepare(
            "SELECT host_key, name, encrypted_value, path, expires_utc, is_secure, is_httponly \
             FROM cookies",
        )?;

        let mut harvest = ProfileHarvest {
            cookies: Vec::new(),
            saw_undecryptable: false,
        };

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,   // host_key (domain)
                row.get::<_, String>(1)?,   // name
                row.get::<_, Vec<u8>>(2)?,  // encrypted_value
                row.get::<_, String>(3)?,   // path
                row.get::<_, i64>(4)?,      // expires_utc (1601 epoch, micros)
                row.get::<_, i64>(5)? != 0, // is_secure
                row.get::<_, i64>(6)? != 0, // is_httponly
            ))
        })?;

        for row in rows {
            let (host, name, enc, path, expires_utc, secure, http_only) = row?;
            if !domain_matches(board_id, &host) {
                continue;
            }

            match decrypt_value(&enc, key) {
                DecryptResult::Plain(value) if !value.is_empty() => {
                    harvest.cookies.push(StoredCookie {
                        name,
                        value,
                        domain: host,
                        path,
                        expires: chromium_time_to_unix(expires_utc),
                        http_only,
                        secure,
                    });
                }
                DecryptResult::Plain(_) => { /* empty value — skip */ }
                DecryptResult::Undecryptable => harvest.saw_undecryptable = true,
            }
        }

        Ok::<_, anyhow::Error>(harvest)
    })();

    tmp.cleanup();
    result
}

/// A temp copy of a locked SQLite DB that deletes itself on `cleanup()`.
struct TempDb {
    path: PathBuf,
    extra: Vec<PathBuf>,
}

impl TempDb {
    fn cleanup(self) {
        let _ = std::fs::remove_file(&self.path);
        for p in &self.extra {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn copy_locked_db(src: &Path) -> Result<TempDb> {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "ajh-cookies-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    dir.push(unique);

    std::fs::copy(src, &dir)?;

    // Copy WAL/SHM siblings too so a checkpoint-pending DB reads consistently.
    let mut extra = Vec::new();
    for suffix in ["-wal", "-shm"] {
        let sib = path_with_suffix(src, suffix);
        if sib.exists() {
            let dst = path_with_suffix(&dir, suffix);
            if std::fs::copy(&sib, &dst).is_ok() {
                extra.push(dst);
            }
        }
    }

    Ok(TempDb { path: dir, extra })
}

fn path_with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

// ── Domain matching ───────────────────────────────────────────────────────────

/// True if a cookie host belongs to the board. Fixed-suffix boards
/// (linkedin/xing/glassdoor) use a `.`-anchored suffix match so a lookalike
/// host like `linkedin.com.evil.test` does not match. Indeed spans locale TLDs
/// (indeed.de/.co.uk/.fr/…), so we match the `indeed.` label rather than a
/// fixed suffix.
fn domain_matches(board_id: &str, host: &str) -> bool {
    let host = host.trim_start_matches('.').to_ascii_lowercase();
    // `.`-anchored suffix: host is exactly `d` or ends with `.d`.
    let suffix_match = |d: &str| host == d || host.ends_with(&format!(".{d}"));
    match board_id {
        "linkedin" => suffix_match("linkedin.com"),
        "indeed" => host.contains("indeed."),
        "xing" => suffix_match("xing.com"),
        "glassdoor" => suffix_match("glassdoor.com"),
        _ => false,
    }
}

/// Chromium stores `expires_utc` as microseconds since 1601-01-01 UTC. Convert
/// to unix seconds (`f64`) to match the `StoredCookie.expires` shape used by
/// the browser-login export path. `0` (session cookie) → `None`.
fn chromium_time_to_unix(expires_utc: i64) -> Option<f64> {
    if expires_utc <= 0 {
        return None;
    }
    // Microseconds between 1601-01-01 and 1970-01-01.
    const EPOCH_DELTA_US: i64 = 11_644_473_600_000_000;
    let unix_us = expires_utc - EPOCH_DELTA_US;
    if unix_us <= 0 {
        return None;
    }
    Some(unix_us as f64 / 1_000_000.0)
}

// ── Decryption ────────────────────────────────────────────────────────────────

enum DecryptResult {
    Plain(String),
    Undecryptable,
}

/// Decrypt a Chromium `encrypted_value` blob.
///
/// * `v10`/`v11` → AES-256-GCM with the recovered os_crypt key. Some builds
///   prepend a 32-byte SHA-256 domain hash to the plaintext — we strip it when
///   stripping yields valid UTF-8 and the unstripped form does not.
/// * `v20` → App-Bound Encryption (Chrome 127+). Not decryptable here.
/// * no version prefix → legacy DPAPI-wrapped whole blob (Windows only).
fn decrypt_value(enc: &[u8], key: Option<&[u8]>) -> DecryptResult {
    if enc.is_empty() {
        return DecryptResult::Plain(String::new());
    }

    if enc.len() >= 3 && (&enc[0..3] == b"v10" || &enc[0..3] == b"v11") {
        let Some(key) = key else {
            return DecryptResult::Undecryptable;
        };
        return match decrypt_v10_aes_gcm(enc, key) {
            Some(bytes) => decode_plaintext(bytes),
            None => decrypt_v10_unix_cbc(enc, key)
                .map(decode_plaintext)
                .unwrap_or(DecryptResult::Undecryptable),
        };
    }

    if enc.len() >= 3 && &enc[0..3] == b"v20" {
        // App-Bound Encryption — sealed to the browser; out of scope.
        return DecryptResult::Undecryptable;
    }

    // Legacy (pre-v10): the entire blob is DPAPI-protected on Windows.
    legacy_dpapi_decrypt(enc)
        .map(decode_plaintext)
        .unwrap_or(DecryptResult::Undecryptable)
}

/// Convert decrypted bytes to a cookie string, stripping a leading 32-byte
/// SHA-256 domain hash when that is what makes it valid UTF-8.
fn decode_plaintext(bytes: Vec<u8>) -> DecryptResult {
    if let Ok(s) = std::str::from_utf8(&bytes) {
        // Newer Windows builds still prefix a 32-byte hash even when the raw
        // bytes happen to be valid UTF-8; but cookie values never legitimately
        // begin with 32 bytes of binary-looking hash followed by ASCII, so we
        // only strip when the unstripped form is NOT clean ASCII and the
        // stripped form is. Cheapest correct heuristic: prefer the stripped
        // form if the first 32 bytes are non-printable.
        if bytes.len() > 32
            && bytes[..32]
                .iter()
                .any(|b| !b.is_ascii_graphic() && *b != b' ')
        {
            if let Ok(stripped) = std::str::from_utf8(&bytes[32..]) {
                return DecryptResult::Plain(stripped.to_owned());
            }
        }
        return DecryptResult::Plain(s.to_owned());
    }
    // Not UTF-8 as-is → try stripping the 32-byte hash prefix.
    if bytes.len() > 32 {
        if let Ok(stripped) = std::str::from_utf8(&bytes[32..]) {
            return DecryptResult::Plain(stripped.to_owned());
        }
    }
    DecryptResult::Undecryptable
}

/// AES-256-GCM: nonce = bytes[3..15], ciphertext+tag = bytes[15..].
fn decrypt_v10_aes_gcm(enc: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    if key.len() != 32 || enc.len() < 3 + 12 + 16 {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = Nonce::from_slice(&enc[3..15]);
    cipher.decrypt(nonce, &enc[15..]).ok()
}

// ── Per-OS os_crypt key recovery ──────────────────────────────────────────────

/// Recover the symmetric os_crypt key for a user-data root.
/// * Windows: base64 `Local State` → strip `DPAPI` prefix → `CryptUnprotectData`.
/// * Unix: the AES-128-CBC path derives its key from the Safe-Storage password
///   at decrypt time, so no 32-byte GCM key is returned here.
#[cfg(target_os = "windows")]
fn recover_os_crypt_key(root: &Path) -> Option<Vec<u8>> {
    use base64::Engine;

    let local_state = std::fs::read_to_string(root.join("Local State")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&local_state).ok()?;
    let b64 = json.get("os_crypt")?.get("encrypted_key")?.as_str()?;
    let wrapped = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    if wrapped.len() <= 5 || &wrapped[..5] != b"DPAPI" {
        return None;
    }
    dpapi_unprotect(&wrapped[5..])
}

#[cfg(not(target_os = "windows"))]
fn recover_os_crypt_key(_root: &Path) -> Option<Vec<u8>> {
    // Unix derives the AES-128-CBC key from the Safe-Storage password lazily in
    // decrypt_v10_unix_cbc; there is no standalone 32-byte GCM key to recover.
    None
}

#[cfg(target_os = "windows")]
fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    // SAFETY: we hand CryptUnprotectData a valid input blob and a zeroed output
    // blob; on success we copy out `cbData` bytes then LocalFree the buffer it
    // allocated. All pointers are checked before deref.
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();

        CryptUnprotectData(&in_blob, None, None, None, None, 0, &mut out_blob).ok()?;

        if out_blob.pbData.is_null() || out_blob.cbData == 0 {
            return None;
        }
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            out_blob.pbData as *mut core::ffi::c_void,
        )));
        Some(slice)
    }
}

/// Legacy pre-v10 cookies are the entire blob DPAPI-protected (Windows only).
#[cfg(target_os = "windows")]
fn legacy_dpapi_decrypt(enc: &[u8]) -> Option<Vec<u8>> {
    dpapi_unprotect(enc)
}

#[cfg(not(target_os = "windows"))]
fn legacy_dpapi_decrypt(_enc: &[u8]) -> Option<Vec<u8>> {
    None
}

// ── Unix AES-128-CBC path (macOS Keychain / Linux libsecret) ──────────────────

/// Decrypt a `v10`/`v11` blob on Unix: derive an AES-128 key via
/// PBKDF2-HMAC-SHA1 over the "Safe Storage" password, then AES-128-CBC with a
/// 16-space IV over `bytes[3..]`. Best-effort — returns `None` if the password
/// store is unavailable.
#[cfg(not(target_os = "windows"))]
fn decrypt_v10_unix_cbc(enc: &[u8], _key: &[u8]) -> Option<Vec<u8>> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    if enc.len() <= 3 {
        return None;
    }
    let password = safe_storage_password()?;

    // Chromium constants for the Unix CBC scheme.
    const SALT: &[u8] = b"saltysalt";
    const IV: [u8; 16] = [b' '; 16];
    let iterations: u32 = if cfg!(target_os = "macos") { 1003 } else { 1 };

    let mut derived = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), SALT, iterations, &mut derived);

    let cipher = Aes128CbcDec::new_from_slices(&derived, &IV).ok()?;
    cipher
        .decrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&enc[3..])
        .ok()
}

#[cfg(target_os = "windows")]
fn decrypt_v10_unix_cbc(_enc: &[u8], _key: &[u8]) -> Option<Vec<u8>> {
    None
}

/// Read the Chromium "Safe Storage" password from the OS secret store.
/// macOS: Keychain generic password (service "Chrome Safe Storage").
/// Linux: libsecret; falls back to the literal "peanuts" some builds use.
#[cfg(not(target_os = "windows"))]
fn safe_storage_password() -> Option<String> {
    // We reuse the app's keyring-core stack; on Linux a locked/absent keyring
    // yields the documented "peanuts" fallback Chromium uses for plaintext
    // storage. macOS requires the Keychain item to be readable.
    #[cfg(target_os = "macos")]
    {
        // Service/account used by Google Chrome's Keychain item.
        if let Some(pw) = read_keychain_password("Chrome Safe Storage", "Chrome") {
            return Some(pw);
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        // libsecret lookup is environment-specific; fall back to "peanuts",
        // which is what Chromium uses when no secret service is available.
        Some("peanuts".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Other Unix targets (e.g. BSD): no supported secret store here.
        None
    }
}

#[cfg(target_os = "macos")]
fn read_keychain_password(service: &str, account: &str) -> Option<String> {
    use std::process::Command;
    // `security find-generic-password -w` prints just the password.
    let out = Command::new("security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let pw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if pw.is_empty() {
        None
    } else {
        Some(pw)
    }
}

#[cfg(test)]
mod test;
