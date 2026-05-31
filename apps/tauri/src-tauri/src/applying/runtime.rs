//! Shared chromiumoxide runtime for appliers.
//!
//! Opens a headed Chromium with the board's persistent profile (so the user's
//! saved login from `board_login::open_login` is reused) and navigates to the
//! posting URL. Returns the live page so each applier can drive its own form
//! flow. The caller owns the `Browser` and must `close()` it when done.

use anyhow::{anyhow, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct ApplySession {
    pub browser: Browser,
    pub page: Page,
    pub closed: Arc<AtomicBool>,
}

impl ApplySession {
    /// Launch Chromium with the board's persistent profile and open the URL.
    ///
    /// `headless`: if true, run in headless mode (no window). Default false for
    /// apply flows (user interaction needed). Set true for future scraping flows
    /// that need JS rendering without showing a window.
    pub async fn open(
        app_data_dir: &Path,
        board_id: &str,
        posting_url: &str,
        headless: bool,
    ) -> Result<ApplySession> {
        let profile = crate::scraping::board_login::profile_dir(app_data_dir, board_id);
        std::fs::create_dir_all(&profile).ok();

        let mut builder = BrowserConfig::builder();
        if !headless {
            builder = builder.with_head();
        }
        builder = builder
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--no-default-browser-check")
            .arg("--no-first-run");

        // Use system Chrome/Edge if available.
        if let Some(chrome_path) = crate::platform::detect_system_chrome() {
            builder = builder.chrome_executable(chrome_path);
        }

        let config = builder
            .build()
            .map_err(|e| anyhow!("BrowserConfig build failed: {e}"))?;

        let (browser, mut handler) = Browser::launch(config).await?;

        let closed = Arc::new(AtomicBool::new(false));
        let closed_clone = closed.clone();
        tokio::spawn(async move {
            while handler.next().await.is_some() {}
            closed_clone.store(true, Ordering::SeqCst);
        });

        let page = browser.new_page(posting_url).await?;
        Ok(ApplySession {
            browser,
            page,
            closed,
        })
    }

    pub async fn close(mut self) {
        let _ = self.browser.close().await;
    }
}
