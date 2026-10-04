//! Board authentication flow.
//!
//! Opens a headed Chromium
//! window via `chromiumoxide` pointed at the board's login URL. Auth is
//! detected by URL pattern and/or cookie predicates per board. On success
//! the cookies are exported to `<board-state>/cookies.json` so HTTP scrapers
//! can reuse the session without launching the browser again.
//!
//! ── Layout on disk ───────────────────────────────────────────────────────
//! <app_data_dir>/browser-state/<board_id>/
//!   ├── profile/             ← Chromium --user-data-dir (cookies, storage)
//!   ├── cookies.json         ← exported cookies for reqwest cookie jar
//!   └── auth-status.json     ← { connected, connected_at }

mod browser_flow;
mod import;
pub use browser_flow::open_login;
pub use import::{import_cookies, ImportOutcome};

use anyhow::{anyhow, Result};
use chromiumoxide::Page;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::platform::fs::write_atomic;

pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

// ── Board configs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct BoardLoginConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub login_url: &'static str,
    /// True when the URL indicates a logged-in page (default heuristic if None).
    pub is_authed_url: Option<fn(&str) -> bool>,
    /// True when the cookie jar contains the auth marker(s) for this board.
    pub is_authed_cookies: Option<fn(&[StoredCookie]) -> bool>,
}

const CONFIGS: &[BoardLoginConfig] = &[
    BoardLoginConfig {
        id: "linkedin",
        display_name: "LinkedIn",
        login_url: "https://www.linkedin.com/login",
        is_authed_url: None,
        is_authed_cookies: Some(|cookies| {
            cookies.iter().any(|c| {
                c.name == "li_at" && c.value.len() > 10 && c.domain.contains("linkedin.com")
            })
        }),
    },
    BoardLoginConfig {
        id: "indeed",
        display_name: "Indeed",
        login_url: "https://secure.indeed.com/auth",
        is_authed_url: Some(|u| {
            u.contains("indeed.com") && !u.contains("/auth") && !u.contains("/login")
        }),
        is_authed_cookies: None,
    },
    BoardLoginConfig {
        id: "xing",
        display_name: "Xing",
        login_url: "https://login.xing.com/login",
        is_authed_url: Some(|u| {
            u.contains("xing.com") && !u.contains("login.xing.com") && !u.contains("/login")
        }),
        is_authed_cookies: None,
    },
    BoardLoginConfig {
        id: "glassdoor",
        display_name: "Glassdoor",
        login_url: "https://www.glassdoor.com/profile/login_input.htm",
        is_authed_url: Some(|u| {
            u.contains("glassdoor.com")
                && !u.contains("/profile/login")
                && !u.contains("/index.htm?sso")
        }),
        is_authed_cookies: None,
    },
];

pub fn get_config(board_id: &str) -> Option<&'static BoardLoginConfig> {
    CONFIGS.iter().find(|c| c.id == board_id)
}

pub fn default_is_authed_url(url: &str) -> bool {
    !url.contains("/login")
        && !url.contains("/auth")
        && !url.contains("/signin")
        && !url.contains("/checkpoint")
        && !url.contains("/uas/")
}

// ── Persisted cookies ───────────────────────────────────────────────────────

/// Subset of the CDP `Cookie` struct that we persist for reqwest reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
    pub http_only: bool,
    pub secure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthStatus {
    connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    connected_at: Option<u64>,
}

// ── Paths ───────────────────────────────────────────────────────────────────

pub fn board_state_dir(app_data_dir: &Path, board_id: &str) -> PathBuf {
    app_data_dir.join("browser-state").join(board_id)
}

pub fn profile_dir(app_data_dir: &Path, board_id: &str) -> PathBuf {
    board_state_dir(app_data_dir, board_id).join("profile")
}

pub fn cookies_path(app_data_dir: &Path, board_id: &str) -> PathBuf {
    board_state_dir(app_data_dir, board_id).join("cookies.json")
}

pub fn auth_status_path(app_data_dir: &Path, board_id: &str) -> PathBuf {
    board_state_dir(app_data_dir, board_id).join("auth-status.json")
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Check the persisted auth status without opening a browser.
pub fn get_status(app_data_dir: &Path, board_id: &str) -> bool {
    std::fs::read_to_string(auth_status_path(app_data_dir, board_id))
        .ok()
        .and_then(|s| serde_json::from_str::<AuthStatus>(&s).ok())
        .map(|s| s.connected)
        .unwrap_or(false)
}

/// Age of the persisted login in milliseconds, or `None` if no session exists.
/// Lets the UI display "expires soon" warnings before scrapers start failing.
pub fn session_age_ms(app_data_dir: &Path, board_id: &str) -> Option<u64> {
    let status: AuthStatus = std::fs::read_to_string(auth_status_path(app_data_dir, board_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    if !status.connected {
        return None;
    }
    let connected_at = status.connected_at?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    Some(now.saturating_sub(connected_at))
}

/// Soft cap on how long we trust an authenticated session before treating it
/// as stale and asking the user to re-login. 7 days is the LinkedIn / Indeed
/// real-world floor before cookies start rotating.
pub const SESSION_MAX_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Returns true if the persisted session is older than `SESSION_MAX_AGE_MS`.
pub fn session_is_stale(app_data_dir: &Path, board_id: &str) -> bool {
    session_age_ms(app_data_dir, board_id)
        .map(|age| age > SESSION_MAX_AGE_MS)
        .unwrap_or(false)
}

/// Bump `connected_at` to "now" — call after a successful authenticated
/// request so the session-stale countdown restarts. Mirrors Playwright's
/// "context.storageState()" refresh on each navigation.
pub fn touch_session(app_data_dir: &Path, board_id: &str) {
    if !get_status(app_data_dir, board_id) {
        return;
    }
    write_auth_status(app_data_dir, board_id, true);
}

/// Clear the board's session. User will need to log in again.
pub fn disconnect(app_data_dir: &Path, board_id: &str) {
    // Write disconnected status explicitly — more reliable than nuking the
    // profile directory (Chromium may have files locked on Windows).
    write_auth_status(app_data_dir, board_id, false);
    let _ = std::fs::remove_file(cookies_path(app_data_dir, board_id));
}

// ── Internals ───────────────────────────────────────────────────────────────

async fn read_cookies(page: &Page) -> Result<Vec<StoredCookie>> {
    let cookies = page
        .get_cookies()
        .await
        .map_err(|e| anyhow!("get_cookies failed: {e}"))?;
    Ok(cookies
        .iter()
        .map(|c| StoredCookie {
            name: c.name.clone(),
            value: c.value.clone(),
            domain: c.domain.clone(),
            path: c.path.clone(),
            expires: Some(c.expires),
            http_only: c.http_only,
            secure: c.secure,
        })
        .collect())
}

async fn export_cookies(page: &Page, app_data_dir: &Path, board_id: &str) -> Result<()> {
    let cookies = read_cookies(page).await?;
    write_cookies(app_data_dir, board_id, &cookies)
}

/// Persist `cookies` to `<board-state>/cookies.json` in the exact format the
/// HTTP scrapers consume (`Vec<StoredCookie>`). Shared by the browser-login
/// export path and the cookie-import path (`import.rs`) so both produce
/// byte-identical artifacts.
pub(crate) fn write_cookies(
    app_data_dir: &Path,
    board_id: &str,
    cookies: &[StoredCookie],
) -> Result<()> {
    let path = cookies_path(app_data_dir, board_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cookies)?;
    write_atomic(&path, json.as_bytes())?;
    Ok(())
}

pub(crate) fn write_auth_status(app_data_dir: &Path, board_id: &str, connected: bool) {
    let path = auth_status_path(app_data_dir, board_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let status = AuthStatus {
        connected,
        connected_at: if connected {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            )
        } else {
            None
        },
    };
    if let Ok(json) = serde_json::to_string(&status) {
        let _ = write_atomic(&path, json.as_bytes());
    }
}

/// Load persisted cookies from `<board-state>/cookies.json`. Empty when the
/// user has not logged in yet.
pub fn load_cookies(app_data_dir: &Path, board_id: &str) -> Vec<StoredCookie> {
    std::fs::read_to_string(cookies_path(app_data_dir, board_id))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<StoredCookie>>(&s).ok())
        .unwrap_or_default()
}

/// Rebuild a `reqwest` cookie jar from persisted `cookies`, honouring each
/// cookie's own `Domain` scope instead of collapsing everything to host-only.
///
/// RFC 6265 §5.2.3: a cookie added with no `Domain` attribute is host-only —
/// bound to the exact host of the URL it's added against. Chromium's exported
/// `domain` field carries a leading dot when the cookie was captured *with* a
/// `Domain` attribute (subdomain-scoped) and no dot when it was host-only. We
/// mirror that distinction back into the rebuilt cookie string so a
/// `.example.com` cookie still reaches `www.example.com`, and a host-only
/// cookie stays bound to exactly the host it was issued for.
///
/// Deliberately narrow, not wide: we only ever set `Domain` to the value the
/// site itself set (`host`, derived from the stored domain), and the jar's
/// own domain-match check against `url` (also derived from `host`) still
/// rejects anything that doesn't match — this can't send a cookie to a host
/// it wasn't issued for.
pub(crate) fn build_cookie_jar(cookies: &[StoredCookie]) -> std::sync::Arc<reqwest::cookie::Jar> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());

    for c in cookies {
        // Domains starting with '.' are valid in netscape format but need a
        // concrete host for the URL. Use https://<domain-no-leading-dot>/.
        let host = c.domain.trim_start_matches('.');
        if host.is_empty() {
            continue;
        }
        let url = match reqwest::Url::parse(&format!("https://{host}/")) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let mut cookie_str = format!("{}={}; Path={}", c.name, c.value, c.path);
        // A leading dot on the stored domain means the cookie was captured with
        // an explicit Domain attribute (subdomain-scoped). Add that Domain
        // attribute back so the jar treats it as a domain cookie (sent to
        // `host` and its subdomains) instead of silently downgrading it to
        // host-only. No leading dot means it was already host-only when
        // captured — omit Domain so it stays bound to exactly `host`.
        if c.domain.starts_with('.') {
            cookie_str.push_str(&format!("; Domain={host}"));
        }
        if c.secure {
            cookie_str.push_str("; Secure");
        }
        if c.http_only {
            cookie_str.push_str("; HttpOnly");
        }
        jar.add_cookie_str(&cookie_str, &url);
    }

    jar
}

/// Build an authenticated reqwest::Client for `board_id`. The returned client
/// has a cookie jar pre-populated with the cookies captured during login.
///
/// Returns an empty-jar client if no cookies are stored — callers can decide
/// whether to fall through to a guest flow or surface a "not connected" error.
pub fn build_authed_client(app_data_dir: &Path, board_id: &str) -> Result<reqwest::Client> {
    let jar = build_cookie_jar(&load_cookies(app_data_dir, board_id));

    crate::net::http::build_client(crate::net::http::ClientConfig {
        timeout: Some(std::time::Duration::from_secs(30)),
        cookie_jar: Some(jar),
    })
    .map_err(|e| anyhow!("reqwest client build failed: {e}"))
}

#[cfg(test)]
mod tests;
