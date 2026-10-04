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
/// alone would also match `evillinkedin.com` (missing the `.` boundary), and
/// `strip_suffix` (rather than `ends_with`) additionally rejects an EMPTY
/// leading label — `https://.linkedin.com/in/x` — which `ends_with` alone
/// would accept.
///
/// Defence in depth, not the load-bearing control: `net::http::get_guarded*`
/// already rejects any non-HTTP(S) scheme before connecting, so a
/// `file://`/`foo://` URL can never actually reach the network. Checking the
/// scheme here too just keeps this function honest about what it classifies
/// as LinkedIn, independent of what a later layer would have done with it.
fn detect_platform(url: &str) -> Option<Platform> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let is_linkedin_host = host == "linkedin.com"
        || host
            .strip_suffix(".linkedin.com")
            .is_some_and(|label| !label.is_empty());
    if is_linkedin_host && parsed.path().to_ascii_lowercase().contains("/in/") {
        Some(Platform::LinkedIn)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
