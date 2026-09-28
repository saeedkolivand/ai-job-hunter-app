//! Headed-Chromium login flow: launch a per-board persistent profile,
//! detect success by URL pattern and/or cookie predicate, then export
//! cookies through the shared [`super`] persistence helpers.

use std::time::Duration;

use anyhow::{anyhow, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;

use crate::observability::sanitize_reason;

use super::*;

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
    // Flatpak browsers cannot be passed as a raw binary path to chromiumoxide
    // (they require `flatpak run <id>`), so we only set the executable for
    // installs that expose a native binary path.
    if let Some(launch) = crate::platform::detect_system_chrome() {
        if let Some(chrome_path) = launch.to_executable_path() {
            builder = builder.chrome_executable(chrome_path);
        }
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
            log::warn!(
                "[board_login] failed to export cookies for {board_id}: {}",
                sanitize_reason(&e.to_string())
            );
        }
    } else {
        on_status("Login cancelled or timed out");
    }

    // Close the browser cleanly. Ignore errors — the user may have closed it.
    let _ = tokio::time::timeout(Duration::from_secs(5), browser.close()).await;

    write_auth_status(app_data_dir, board_id, connected);
    Ok(connected)
}

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
