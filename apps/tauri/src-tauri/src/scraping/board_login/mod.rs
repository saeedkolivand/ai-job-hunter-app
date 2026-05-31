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

use anyhow::{anyhow, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use chromiumoxide::Page;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// Open a headed Chromium window for the board's login flow.
///
/// Returns true if the user successfully authenticated, false if the window
/// closed or the timeout elapsed. Cookies are exported to `cookies.json` and
/// `auth-status.json` is updated either way.
pub async fn open_login<F>(app_data_dir: &Path, board_id: &str, on_status: F) -> Result<bool>
where
    F: Fn(&str) + Send + Sync,
{
    let config =
        get_config(board_id).ok_or_else(|| anyhow!("No login config for board: {board_id}"))?;

    let profile = profile_dir(app_data_dir, board_id);
    std::fs::create_dir_all(&profile).ok();

    on_status(&format!("Opening {} login window…", config.display_name));

    // Launch headed Chromium with a per-board persistent profile.
    let mut builder = BrowserConfig::builder()
        .with_head()
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--no-default-browser-check")
        .arg("--no-first-run");

    // Use system Chrome/Edge if available to avoid chromiumoxide's 120 MB download.
    if let Some(chrome_path) = crate::platform::detect_system_chrome() {
        builder = builder.chrome_executable(chrome_path);
    }

    let browser_config = builder
        .build()
        .map_err(|e| anyhow!("BrowserConfig build failed: {e}"))?;

    let (mut browser, mut handler) = Browser::launch(browser_config).await?;

    // Drive the CDP event loop in the background. When the user closes the
    // window the handler stream ends — we surface that as a cancellation flag.
    let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let closed_clone = closed.clone();
    tokio::spawn(async move {
        while handler.next().await.is_some() {}
        closed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    let page = browser.new_page(config.login_url).await?;

    // LinkedIn pushes passkeys aggressively — block WebAuthn so password login works.
    if config.id == "linkedin" {
        let _ = page.evaluate_on_new_document(DISABLE_PASSKEY_SCRIPT).await;
        let _ = page.reload().await;
    }

    let connected = wait_for_auth(&page, config, &closed).await;

    if connected {
        on_status("Login successful, exporting cookies…");
        if let Err(e) = export_cookies(&page, app_data_dir, board_id).await {
            log::warn!("[board_login] failed to export cookies for {board_id}: {e}");
        }
    } else {
        on_status("Login cancelled or timed out");
    }

    // Close the browser cleanly. Ignore errors — the user may have closed it.
    let _ = tokio::time::timeout(Duration::from_secs(5), browser.close()).await;

    write_auth_status(app_data_dir, board_id, connected);
    Ok(connected)
}

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

async fn wait_for_auth(
    page: &Page,
    config: &BoardLoginConfig,
    closed: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < LOGIN_TIMEOUT {
        if closed.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }

        // URL check - if this fails, the browser was likely closed
        let url = match page.url().await {
            Ok(Some(u)) => u,
            Ok(None) => continue,
            Err(_) => return false, // Browser closed or disconnected
        };

        let url_ok = match config.is_authed_url {
            Some(f) => f(&url),
            None => default_is_authed_url(&url),
        };
        // Only trust URL when the page has left the login URL.
        if url_ok && !url.starts_with(config.login_url) && config.is_authed_cookies.is_none() {
            return true;
        }

        // Cookie check (preferred for AJAX-login boards like LinkedIn).
        if let Some(predicate) = config.is_authed_cookies {
            if let Ok(cookies) = read_cookies(page).await {
                if predicate(&cookies) {
                    return true;
                }
            }
            // If read_cookies fails, browser might be closed
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
    false
}

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
    let path = cookies_path(app_data_dir, board_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&cookies)?;
    std::fs::write(&path, json)?;
    Ok(())
}

fn write_auth_status(app_data_dir: &Path, board_id: &str, connected: bool) {
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
        std::fs::write(&path, json).ok();
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

/// Build an authenticated reqwest::Client for `board_id`. The returned client
/// has a cookie jar pre-populated with the cookies captured during login.
///
/// Returns an empty-jar client if no cookies are stored — callers can decide
/// whether to fall through to a guest flow or surface a "not connected" error.
pub fn build_authed_client(app_data_dir: &Path, board_id: &str) -> Result<reqwest::Client> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());

    for c in load_cookies(app_data_dir, board_id) {
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
        if c.secure {
            cookie_str.push_str("; Secure");
        }
        if c.http_only {
            cookie_str.push_str("; HttpOnly");
        }
        jar.add_cookie_str(&cookie_str, &url);
    }

    crate::net::http::build_client(crate::net::http::ClientConfig {
        timeout: Some(std::time::Duration::from_secs(30)),
        cookie_jar: Some(jar),
    })
    .map_err(|e| anyhow!("reqwest client build failed: {e}"))
}

/// Convenience: build cookie param vec for chromiumoxide page restoration.
#[allow(dead_code)]
pub fn to_cookie_params(cookies: &[StoredCookie]) -> Vec<CookieParam> {
    cookies
        .iter()
        .map(|c| {
            let mut p = CookieParam::new(c.name.clone(), c.value.clone());
            p.domain = Some(c.domain.clone());
            p.path = Some(c.path.clone());
            p.http_only = Some(c.http_only);
            p.secure = Some(c.secure);
            p
        })
        .collect()
}

pub const DISABLE_PASSKEY_SCRIPT: &str = r#"
(function () {
  try {
    const orig = navigator.credentials;
    if (!orig) return;
    const origGet = orig.get.bind(orig);
    const origCreate = orig.create.bind(orig);
    const notAllowed = () =>
      Promise.reject(
        Object.assign(new DOMException('User cancelled', 'NotAllowedError'), { code: 20 })
      );
    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      get: () => ({
        get: (o) => (o && o.publicKey ? notAllowed() : origGet(o)),
        create: (o) => (o && o.publicKey ? notAllowed() : origCreate(o)),
        store: orig.store.bind(orig),
        preventSilentAccess: orig.preventSilentAccess.bind(orig),
      }),
    });
  } catch (_) {}
})();
"#;

#[cfg(test)]
mod test;
