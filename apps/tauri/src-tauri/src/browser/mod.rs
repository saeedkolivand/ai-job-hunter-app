// DORMANT SUBSYSTEM — zero live callers outside this module's own tests.
// `BrowserController` was designed for CDP-based scraping but has never been
// wired into the SCRAPERS registry; all current board scrapers use the
// `scraping::http` path instead.  Kept for future browser-automation scrapers
// (e.g. sites that require JS rendering).  Remove only after confirming no
// planned scraper needs it — see M2 audit note.
#![allow(dead_code)]

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
/// Browser controller using chromiumoxide (Chrome DevTools Protocol).
///
/// Lazily spawns Chromium on first use, reuses a single context across scrapers,
/// applies sensible anti-bot defaults, and supports clean shutdown.
use std::sync::Arc;

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct BrowserControllerOptions {
    pub headless: bool,
    pub user_agent: String,
    pub locale: String,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for BrowserControllerOptions {
    fn default() -> Self {
        Self {
            headless: true,
            user_agent: crate::net::http::DEFAULT_UA.to_string(),
            locale: "en-US".to_string(),
            viewport_width: 1366,
            viewport_height: 900,
        }
    }
}

pub struct BrowserController {
    browser: Option<Arc<Browser>>,
    opts: BrowserControllerOptions,
}

impl BrowserController {
    pub fn new(opts: BrowserControllerOptions) -> Self {
        Self {
            browser: None,
            opts,
        }
    }

    pub async fn ensure(&mut self) -> AppResult<Arc<Browser>> {
        if let Some(ref browser) = self.browser {
            return Ok(browser.clone());
        }

        let config = BrowserConfig::builder()
            .with_head()
            .window_size(self.opts.viewport_width, self.opts.viewport_height)
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--no-default-browser-check")
            .arg("--disable-features=IsolateOrigins,site-per-process")
            .arg(format!("--user-agent={}", self.opts.user_agent).as_str())
            .build()
            .map_err(|e| AppError::Network(format!("browser config build failed: {e}")))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| AppError::Network(format!("browser launch failed: {e}")))?;
        let browser = Arc::new(browser);

        // Spawn the handler in the background
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                // Handle browser events if needed
                let _ = event;
            }
        });

        self.browser = Some(browser.clone());
        log::info!("[browser] chromium launched");
        Ok(browser)
    }

    pub async fn with_page<T, F>(&mut self, f: F) -> AppResult<T>
    where
        F: FnOnce(Page) -> futures::future::BoxFuture<'static, AppResult<T>>,
    {
        let browser = self.ensure().await?;
        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| AppError::Network(format!("browser new page failed: {e}")))?;

        // Set anti-detection headers
        page.evaluate("Object.defineProperty(navigator, 'webdriver', { get: () => undefined })")
            .await
            .map_err(|e| AppError::Network(format!("browser evaluate failed: {e}")))?;

        let result = f(page).await;

        // Clean up the page
        // Note: chromiumoxide handles page cleanup when dropped

        result
    }

    pub async fn close(&mut self) -> AppResult<()> {
        if let Some(browser) = self.browser.take() {
            // Arc::try_unwrap to get the original Browser back
            if let Ok(mut browser) = Arc::try_unwrap(browser) {
                browser
                    .close()
                    .await
                    .map_err(|e| AppError::Network(format!("browser close failed: {e}")))?;
                log::warn!("[browser] chromium closed");
            }
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.browser.is_some()
    }
}

#[cfg(test)]
mod test;
