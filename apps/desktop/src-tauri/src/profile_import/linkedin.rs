use scraper::{Html, Selector};
use serde_json::Value;

use super::ProfileData;
use crate::error::{AppError, AppResult};

/// Fetch a LinkedIn public profile page and extract structured data.
///
/// LinkedIn embeds `application/ld+json` blocks with schema.org Person/Organization
/// data on public profile pages. We parse those first; visible HTML sections serve
/// as a fallback for experience, education, and skills.
pub async fn import(url: &str) -> AppResult<ProfileData> {
    let html = fetch_page(url).await?;
    let document = Html::parse_document(&html);

    // Try JSON-LD first for name/headline/summary
    let ld = extract_ld_json(&document);
    let name = ld
        .as_ref()
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(clean);
    let headline = ld
        .as_ref()
        .and_then(|v| v.get("jobTitle"))
        .and_then(|v| v.as_str())
        .map(clean)
        .or_else(|| extract_meta(&document, "og:title").map(|t| strip_linkedin_suffix(&t)));
    let summary = ld
        .as_ref()
        .and_then(|v| v.get("description"))
        .and_then(|v| v.as_str())
        .map(clean);
    let location = ld
        .as_ref()
        .and_then(|v| v.get("address"))
        .and_then(|v| v.get("addressLocality"))
        .and_then(|v| v.as_str())
        .map(clean);

    // Extract lists from HTML structure (visible to crawlers)
    let experience = extract_list_section(&document, "experience");
    let education = extract_list_section(&document, "education");
    let skills = extract_skills(&document, &ld);

    // If JSON-LD name is missing, try og:title
    let name =
        name.or_else(|| extract_meta(&document, "og:title").map(|t| strip_linkedin_suffix(&t)));

    if name.is_none() && experience.is_empty() && skills.is_empty() {
        return Err(AppError::Parse(
            "could not extract profile data — the profile may be private; log in to import it"
                .to_string(),
        ));
    }

    Ok(ProfileData {
        name,
        headline,
        summary,
        experience,
        education,
        skills,
        location,
        platform: "linkedin".to_string(),
    })
}

// ── HTTP ─────────────────────────────────────────────────────────────────────

async fn fetch_page(url: &str) -> AppResult<String> {
    // Use the connected LinkedIn session when available so a private profile
    // becomes importable after the user logs in. With no session the cookie jar
    // is empty, so public profiles still import with no login required.
    let data_dir = crate::platform::config::data_dir();
    let client = crate::scraping::board_login::build_authed_client(&data_dir, "linkedin")
        .unwrap_or_else(|_| crate::net::http::shared());

    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| format!("fetch profile: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(AppError::Provider(format!(
            "linkedin returned {status} — the profile may be private; log in first"
        )));
    }

    resp.text()
        .await
        .map_err(|e| AppError::Network(format!("read body: {e}")))
}

// ── JSON-LD ───────────────────────────────────────────────────────────────────

fn extract_ld_json(doc: &Html) -> Option<Value> {
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for el in doc.select(&sel) {
        let raw = el.text().collect::<String>();
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            let type_field = v.get("@type").and_then(|t| t.as_str()).unwrap_or("");
            if type_field == "Person" {
                return Some(v);
            }
        }
    }
    None
}

// ── Meta tags ─────────────────────────────────────────────────────────────────

fn extract_meta(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!(r#"meta[property="{property}"]"#)).ok()?;
    doc.select(&sel)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(clean)
}

// ── HTML section extraction ───────────────────────────────────────────────────

/// LinkedIn public pages render section ids like `experience-section` / `education-section`.
/// We grab all text inside the matching section element.
fn extract_list_section(doc: &Html, section_id: &str) -> Vec<String> {
    let candidates = [
        format!("#{section_id}-section"),
        format!("section#{section_id}"),
        format!("[id*={section_id}]"),
        format!("[data-section={section_id}]"),
    ];

    for sel_str in &candidates {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(section) = doc.select(&sel).next() {
                return section
                    .text()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .chunks(3)
                    .map(|c| c.join(" · "))
                    .filter(|s| !s.is_empty())
                    .take(20)
                    .collect();
            }
        }
    }

    Vec::new()
}

fn extract_skills(doc: &Html, ld: &Option<Value>) -> Vec<String> {
    // Try JSON-LD knowsAbout array first
    if let Some(v) = ld {
        if let Some(arr) = v.get("knowsAbout").and_then(|a| a.as_array()) {
            let skills: Vec<String> = arr.iter().filter_map(|s| s.as_str()).map(clean).collect();
            if !skills.is_empty() {
                return skills;
            }
        }
    }

    // Fallback: skills section in HTML
    extract_list_section(doc, "skills")
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_linkedin_suffix(s: &str) -> String {
    s.split('|')
        .next()
        .unwrap_or(s)
        .split('-')
        .next()
        .unwrap_or(s)
        .trim()
        .to_string()
}
