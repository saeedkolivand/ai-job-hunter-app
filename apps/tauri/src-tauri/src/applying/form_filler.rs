#![allow(dead_code)]
//! Generic form-filling helpers. Reserved for the next iteration of the
//! apply flow where we'll drive each board's multi-step form via DOM
//! selectors instead of leaving the user to do it manually.
use anyhow::Result;
use crate::applying::selectors::FormSelectors;
use chromiumoxide::page::Page;

pub struct FormFiller {
    selectors: FormSelectors,
}

impl FormFiller {
    pub fn new(selectors: FormSelectors) -> Self {
        Self { selectors }
    }

    /// Find an element using any of the provided selectors
    async fn find_element(&self, page: &Page, selectors: &[String]) -> Result<Option<chromiumoxide::Element>> {
        for selector in selectors {
            match page.find_element(selector.as_str()).await {
                Ok(element) => return Ok(Some(element)),
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    /// Fill form fields with provided data
    pub async fn fill_name(&self, page: &Page, name: &str) -> Result<()> {
        if let Some(element) = self.find_element(page, &self.selectors.name).await? {
            element.click().await?;
            element.type_str(name).await?;
        }
        Ok(())
    }

    pub async fn fill_email(&self, page: &Page, email: &str) -> Result<()> {
        if let Some(element) = self.find_element(page, &self.selectors.email).await? {
            element.click().await?;
            element.type_str(email).await?;
        }
        Ok(())
    }

    pub async fn fill_phone(&self, page: &Page, phone: &str) -> Result<()> {
        if let Some(element) = self.find_element(page, &self.selectors.phone).await? {
            element.click().await?;
            element.type_str(phone).await?;
        }
        Ok(())
    }

    pub async fn upload_resume(&self, page: &Page, resume_path: &str) -> Result<()> {
        if let Some(element) = self.find_element(page, &self.selectors.resume_upload).await? {
            // File upload in chromiumoxide using CDP
            let file_path = std::path::Path::new(resume_path);
            if !file_path.exists() {
                return Err(anyhow::anyhow!("Resume file not found: {}", resume_path));
            }
            
            // Get the element's remote object ID
            let remote_object_id = element.remote_object_id.clone();
            
            // Use CDP to set file upload files
            // DOM.setFileInputFiles is the correct CDP command for file uploads
            page.execute(
                chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams::builder()
                    .files(vec![resume_path.to_string()])
                    .object_id(remote_object_id)
                    .build()
                    .map_err(|e| anyhow::anyhow!("Failed to build SetFileInputFiles: {e}"))?,
            ).await?;
        }
        Ok(())
    }

    pub async fn fill_cover_letter(&self, page: &Page, cover_letter: &str) -> Result<()> {
        if let Some(element) = self.find_element(page, &self.selectors.cover_letter).await? {
            element.click().await?;
            element.type_str(cover_letter).await?;
        }
        Ok(())
    }

    pub async fn submit(&self, page: &Page) -> Result<bool> {
        if let Some(element) = self.find_element(page, &self.selectors.submit_button).await? {
            element.click().await?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Check if CAPTCHA is present on the page
    pub async fn detect_captcha(&self, page: &Page) -> Result<bool> {
        for selector in &self.selectors.captcha_detection {
            if page.find_element(selector.as_str()).await.is_ok() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
