pub mod github;
pub mod linkedin;

use crate::error::AppResult;

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
pub async fn import_from_url(url: &str) -> AppResult<ProfileData> {
    let platform = detect_platform(url).ok_or_else(|| {
        "unsupported profile URL — only LinkedIn is supported at this time".to_string()
    })?;

    match platform {
        Platform::LinkedIn => linkedin::import(url).await,
    }
}

#[derive(Debug)]
enum Platform {
    LinkedIn,
}

/// Detects the platform from a URL's **host** component — never a substring
/// scan of the whole URL. A substring test would let
/// `https://attacker.example/linkedin.com/in/x` (path-embedded lookalike) or
/// `http://127.0.0.1:9200/linkedin.com/in/x` (loopback egress) both classify
/// as LinkedIn. Exact/suffix host match only, mirroring
/// `scraping::scrape_url::try_linkedin`'s guard: a bare `ends_with("linkedin.com")`
/// alone would also match `evillinkedin.com` (missing the `.` boundary).
fn detect_platform(url: &str) -> Option<Platform> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let is_linkedin_host = host == "linkedin.com" || host.ends_with(".linkedin.com");
    if is_linkedin_host && parsed.path().to_ascii_lowercase().contains("/in/") {
        Some(Platform::LinkedIn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_platform: host match, never a substring scan of the whole URL ──

    #[test]
    fn rejects_linkedin_path_embedded_in_a_hostile_host() {
        // Pre-fix, `lower.contains("linkedin.com/in/")` matched this because
        // the string appears in the PATH, not the host.
        assert!(detect_platform("https://attacker.example/linkedin.com/in/x").is_none());
    }

    #[test]
    fn rejects_loopback_host_with_linkedin_in_the_path() {
        assert!(detect_platform("http://127.0.0.1:9200/linkedin.com/in/x").is_none());
    }

    #[test]
    fn rejects_lookalike_suffix_host() {
        // `evillinkedin.com` — a bare `ends_with("linkedin.com")` (no `.`
        // boundary) would wrongly accept this.
        assert!(detect_platform("https://evillinkedin.com/in/x").is_none());
    }

    #[test]
    fn accepts_bare_apex_host() {
        assert!(matches!(
            detect_platform("https://linkedin.com/in/foo"),
            Some(Platform::LinkedIn)
        ));
    }

    #[test]
    fn accepts_www_host() {
        assert!(matches!(
            detect_platform("https://www.linkedin.com/in/foo"),
            Some(Platform::LinkedIn)
        ));
    }

    #[test]
    fn accepts_a_genuine_subdomain() {
        // A real `*.linkedin.com` subdomain (e.g. a locale mirror) must still
        // match via the suffix branch.
        assert!(matches!(
            detect_platform("https://de.linkedin.com/in/foo"),
            Some(Platform::LinkedIn)
        ));
    }

    #[test]
    fn rejects_non_profile_path_on_the_real_host() {
        assert!(detect_platform("https://www.linkedin.com/jobs/view/123").is_none());
    }

    #[test]
    fn rejects_unparseable_input() {
        assert!(detect_platform("not a url").is_none());
    }
}
