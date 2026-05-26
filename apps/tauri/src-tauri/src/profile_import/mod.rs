pub mod linkedin;

/// Platform-agnostic profile scraping result.
#[derive(Debug)]
pub struct ProfileData {
    pub name: Option<String>,
    pub headline: Option<String>,
    pub summary: Option<String>,
    pub experience: Vec<String>,
    pub education: Vec<String>,
    pub skills: Vec<String>,
    pub location: Option<String>,
    pub platform: String,
}

impl ProfileData {
    /// Render the extracted data as plain-text resume.
    pub fn to_resume_text(&self) -> String {
        let mut out = String::new();

        if let Some(name) = &self.name {
            out.push_str(&format!("# {name}\n"));
        }
        if let Some(headline) = &self.headline {
            out.push_str(&format!("{headline}\n"));
        }
        if let Some(location) = &self.location {
            out.push_str(&format!("{location}\n"));
        }
        out.push('\n');

        if let Some(summary) = &self.summary {
            if !summary.is_empty() {
                out.push_str("## Summary\n");
                out.push_str(summary);
                out.push_str("\n\n");
            }
        }

        if !self.experience.is_empty() {
            out.push_str("## Experience\n");
            for item in &self.experience {
                out.push_str(item);
                out.push('\n');
            }
            out.push('\n');
        }

        if !self.education.is_empty() {
            out.push_str("## Education\n");
            for item in &self.education {
                out.push_str(item);
                out.push('\n');
            }
            out.push('\n');
        }

        if !self.skills.is_empty() {
            out.push_str("## Skills\n");
            out.push_str(&self.skills.join(", "));
            out.push('\n');
        }

        out.trim().to_string()
    }
}

/// Detects the platform from a URL and delegates to the matching provider.
pub async fn import_from_url(url: &str) -> Result<ProfileData, String> {
    let platform = detect_platform(url).ok_or_else(|| {
        "unsupported profile URL — only LinkedIn is supported at this time".to_string()
    })?;

    match platform {
        Platform::LinkedIn => linkedin::import(url).await,
    }
}

enum Platform {
    LinkedIn,
}

fn detect_platform(url: &str) -> Option<Platform> {
    let lower = url.to_lowercase();
    if lower.contains("linkedin.com/in/") {
        Some(Platform::LinkedIn)
    } else {
        None
    }
}
