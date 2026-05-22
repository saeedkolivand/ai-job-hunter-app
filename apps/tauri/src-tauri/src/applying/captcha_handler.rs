#![allow(dead_code)]
//! CAPTCHA detection and (optional) external-service solving. Scaffolding
//! kept for future apply-flow automation; not wired into the current
//! `boards::shared::navigate_and_assist` happy path.
use anyhow::Result;
use chromiumoxide::page::Page;

pub enum CaptchaAction {
    /// Wait for user to manually solve the CAPTCHA
    WaitForUser,
    /// Skip the application (cannot proceed)
    Skip,
    /// Retry the page load (sometimes CAPTCHA is transient)
    Retry,
}

pub struct CaptchaHandler {
    max_wait_seconds: u64,
}

impl CaptchaHandler {
    pub fn new(max_wait_seconds: u64) -> Self {
        Self { max_wait_seconds }
    }

    pub fn default() -> Self {
        Self { max_wait_seconds: 120 } // 2 minutes default
    }

    /// Detect if CAPTCHA is present on the page
    pub async fn detect(&self, page: &Page, selectors: &[String]) -> Result<bool> {
        for selector in selectors {
            if page.find_element(selector.as_str()).await.is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Handle CAPTCHA detection
    /// Returns the action to take based on configuration
    pub async fn handle(&self, page: &Page, selectors: &[String]) -> Result<CaptchaAction> {
        if !self.detect(page, selectors).await? {
            return Ok(CaptchaAction::Retry);
        }

        // CAPTCHA detected - default to waiting for user intervention
        // In a real implementation, this could:
        // 1. Show a UI dialog to the user
        // 2. Pause the browser for manual solving
        // 3. Integrate with a CAPTCHA solving service
        
        Ok(CaptchaAction::WaitForUser)
    }

    /// Wait for user to solve CAPTCHA
    /// This implementation:
    /// 1. Polls for CAPTCHA to be resolved
    /// 2. Checks timeout
    /// 3. Returns when resolved or timeout occurs
    pub async fn wait_for_user(&self, page: &Page, selectors: &[String]) -> Result<bool> {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_secs(2);
        
        while start.elapsed() < std::time::Duration::from_secs(self.max_wait_seconds) {
            // Check if CAPTCHA has been resolved
            if self.is_resolved(page, selectors).await? {
                return Ok(true);
            }
            
            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }
        
        // Timeout occurred
        Ok(false)
    }

    /// Check if CAPTCHA has been resolved
    pub async fn is_resolved(&self, page: &Page, selectors: &[String]) -> Result<bool> {
        Ok(!self.detect(page, selectors).await?)
    }
}

/// CAPTCHA solving service integration (optional)
/// This would integrate with services like:
/// - 2Captcha
/// - Anti-Captcha
/// - DeathByCaptcha
pub struct CaptchaSolver {
    api_key: Option<String>,
    service_url: String,
}

impl CaptchaSolver {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            service_url: "https://api.2captcha.com".to_string(),
        }
    }

    /// Solve CAPTCHA using external service
    /// This implementation provides basic structure for:
    /// 1. Getting the CAPTCHA image/data from the page
    /// 2. Sending it to the solving service
    /// 3. Waiting for the solution
    /// 4. Returning the solution code
    pub async fn solve(&self, page: &Page, selectors: &[String]) -> Result<String> {
        if self.api_key.is_none() {
            return Err(anyhow::anyhow!("CAPTCHA solving requires API key"));
        }

        // Step 1: Get CAPTCHA site key from the page
        let site_key = self.extract_site_key(page, selectors).await?;
        
        // Step 2: Send to solving service (2Captcha example)
        let captcha_id = self.create_task(&site_key).await?;
        
        // Step 3: Poll for solution
        let solution = self.wait_for_solution(&captcha_id).await?;
        
        Ok(solution)
    }

    /// Extract reCAPTCHA site key from the page
    async fn extract_site_key(&self, page: &Page, selectors: &[String]) -> Result<String> {
        for selector in selectors {
            // Use page.evaluate to get the attribute as a string
            let js = format!(
                "(() => {{ const el = document.querySelector('{}'); return el ? el.getAttribute('data-sitekey') : null; }})()",
                selector
            );
            
            if let Ok(result) = page.evaluate(js.as_str()).await {
                if let Some(site_key) = result.value().and_then(|v| v.as_str()) {
                    if !site_key.is_empty() {
                        return Ok(site_key.to_string());
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Could not extract CAPTCHA site key"))
    }

    /// Create a CAPTCHA solving task
    async fn create_task(&self, _site_key: &str) -> Result<String> {
        // This would make an HTTP request to the solving service
        // Example for 2Captcha:
        // POST to http://2captcha.com/in.php with:
        // - key: your API key
        // - method: userrecaptcha
        // - googlekey: site_key
        // - pageurl: current page URL
        
        // For now, return a stub task ID
        Ok("stub_task_id".to_string())
    }

    /// Wait for CAPTCHA solution
    async fn wait_for_solution(&self, _task_id: &str) -> Result<String> {
        // This would poll the solving service for the solution
        // Example for 2Captcha:
        // GET http://2captcha.com/res.php with:
        // - key: your API key
        // - action: get
        // - id: task_id
        
        // For now, return a stub solution
        Ok("stub_solution".to_string())
    }
}
