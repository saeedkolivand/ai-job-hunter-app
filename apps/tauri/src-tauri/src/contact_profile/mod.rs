//! Contact profile — the single source of truth for the document header.
//!
//! Resumes and cover letters used to build their header contact line from links
//! scavenged out of the uploaded résumé by a domain heuristic + document
//! position, which swapped a personal LinkedIn for a company page and a personal
//! site for an employer URL (the URL-swap symptom). The header is now assembled
//! from **named fields** held here — never by index, never from the company-link
//! pool — and the same builder feeds the résumé, cover letter, and DOCX, localized
//! per language.
//!
//! Persistence mirrors [`crate::job_preferences`]: a single-row SQLite settings
//! table. Seeding from an imported résumé uses [`classify_contact_links`], which
//! picks the personal profile/site by name, rejects company / job-board pages, and
//! keeps every other personal link as a labelled extra; the import adds email /
//! phone / location from the deterministic structuring pass. The result is
//! *merged* into the stored profile via [`ContactProfile::fill_empty_from`] —
//! filling only empty fields so a sparse profile is completed while every value
//! the user edited is preserved. It is a *suggestion* the user can edit, never
//! silently trusted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
use crate::error::AppResult;
use crate::extraction::types::Link;
use crate::model::rich::{tokenize_rich, url_label, RichText};

// ── Types ───────────────────────────────────────────────────────────────────

/// A free-text value with optional per-language overrides (e.g. a location that
/// reads "Netherlands" in English documents and "Niederlande" in German ones).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedText {
    /// Value used when no language-specific override matches.
    pub default: String,
    /// ISO-639-1 (`de`, `en`, …) → localized value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_lang: BTreeMap<String, String>,
}

impl LocalizedText {
    /// Resolve for `lang` (its primary subtag), falling back to [`Self::default`].
    pub fn resolve(&self, lang: &str) -> &str {
        let primary = lang.split(['-', '_']).next().unwrap_or(lang).to_lowercase();
        self.by_lang
            .get(&primary)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.default)
    }

    fn is_empty(&self) -> bool {
        self.default.trim().is_empty() && self.by_lang.values().all(|v| v.trim().is_empty())
    }
}

/// One additional labelled link beyond the named platform fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactLink {
    pub label: String,
    pub url: String,
}

/// The header contact fields, by name. Every field is optional so a partial
/// profile still produces a valid (shorter) header. The order the header renders
/// in is fixed by [`Self::header_markdown`], not by field discovery order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocalizedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linkedin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_links: Vec<ContactLink>,
    /// Optional candidate photo as a `data:image/<mime>;base64,<payload>` URI
    /// produced by the photo-upload control.  Stored as-is in the JSON column;
    /// `resolve_photo` validates, sanitises, dimension-caps, and re-encodes it
    /// to PNG before embedding.  File paths are never accepted here — this field
    /// is local-only and is never sent over the network.
    /// `None` → no photo; the templates fall back gracefully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
}

impl ContactProfile {
    /// True when there is nothing to render from (caller should fall back to the
    /// text-derived header).
    pub fn is_effectively_empty(&self) -> bool {
        let location_empty = self
            .location
            .as_ref()
            .map(LocalizedText::is_empty)
            .unwrap_or(true);
        self.email.is_none()
            && self.phone.is_none()
            && self.linkedin.is_none()
            && self.github.is_none()
            && self.website.is_none()
            && self.extra_links.is_empty()
            && location_empty
    }

    /// Build the header contact line as a markdown string, localized for `lang`,
    /// in the canonical order **location | email | phone | LinkedIn | GitHub |
    /// Website | extras**. Links are emitted as `[Label](url)` and the email bare
    /// (the renderers turn it into a `mailto:` link); the existing
    /// [`tokenize_rich`] / `split_urls` machinery makes every part clickable.
    ///
    /// This is the single header builder shared by the résumé, cover letter, and
    /// DOCX paths — there is no other place a header URL is chosen.
    pub fn header_markdown(&self, lang: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(loc) = &self.location {
            let v = loc.resolve(lang);
            if !v.trim().is_empty() {
                parts.push(v.to_string());
            }
        }
        if let Some(email) = non_empty(&self.email) {
            parts.push(email.to_string());
        }
        if let Some(phone) = non_empty(&self.phone) {
            parts.push(phone.to_string());
        }
        if let Some(url) = non_empty(&self.linkedin) {
            parts.push(format!("[LinkedIn]({url})"));
        }
        if let Some(url) = non_empty(&self.github) {
            parts.push(format!("[GitHub]({url})"));
        }
        if let Some(url) = non_empty(&self.website) {
            parts.push(format!("[Website]({url})"));
        }
        for link in &self.extra_links {
            if !link.label.trim().is_empty() && !link.url.trim().is_empty() {
                parts.push(format!("[{}]({})", link.label.trim(), link.url.trim()));
            }
        }
        parts.join(" | ")
    }

    /// The header contact line as [`RichText`] (link runs first-class) for the
    /// model-based résumé / DOCX backends. Empty when the profile has no contact
    /// parts, so the caller keeps the text-derived header.
    pub fn header_rich(&self, lang: &str) -> RichText {
        let md = self.header_markdown(lang);
        if md.is_empty() {
            Vec::new()
        } else {
            tokenize_rich(&md)
        }
    }

    /// Override a document header's contact line from this profile, localized for
    /// `lang`. No-op when the profile has no contact parts, so the caller keeps the
    /// text-derived header. The name is left untouched (handled by generation
    /// metadata); this fixes only the contact/link line (the URL-swap symptom).
    pub fn apply_to_header(&self, header: &mut crate::model::document::HeaderBlock, lang: &str) {
        let rich = self.header_rich(lang);
        if !rich.is_empty() {
            header.contact = rich;
        }
    }

    /// The set of header URLs this profile would render (for validation parity
    /// checks across documents). Email is included as a `mailto:` link.
    pub fn header_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(email) = non_empty(&self.email) {
            out.push(format!("mailto:{email}"));
        }
        for url in [
            non_empty(&self.linkedin),
            non_empty(&self.github),
            non_empty(&self.website),
        ]
        .into_iter()
        .flatten()
        {
            out.push(url.to_string());
        }
        for link in &self.extra_links {
            if !link.url.trim().is_empty() {
                out.push(link.url.trim().to_string());
            }
        }
        out
    }

    /// Fill only the **empty/None** fields of `self` from `other`, never
    /// overwriting a value the user already set, and merge in any of `other`'s
    /// extra links that `self` does not already have (by URL). This lets an
    /// import complete a sparse profile (e.g. add the résumé's email / phone /
    /// location / Dribbble) while preserving every field the user edited.
    pub fn fill_empty_from(&mut self, other: &ContactProfile) {
        fn fill(slot: &mut Option<String>, src: &Option<String>) {
            if non_empty(slot).is_none() {
                if let Some(v) = non_empty(src) {
                    *slot = Some(v.to_string());
                }
            }
        }
        fill(&mut self.email, &other.email);
        fill(&mut self.phone, &other.phone);
        fill(&mut self.linkedin, &other.linkedin);
        fill(&mut self.github, &other.github);
        fill(&mut self.website, &other.website);

        if self
            .location
            .as_ref()
            .map(LocalizedText::is_empty)
            .unwrap_or(true)
        {
            if let Some(loc) = &other.location {
                if !loc.is_empty() {
                    self.location = Some(loc.clone());
                }
            }
        }

        for link in &other.extra_links {
            let url = link.url.trim();
            if url.is_empty() || self.extra_links.iter().any(|e| e.url.trim() == url) {
                continue;
            }
            self.extra_links.push(link.clone());
        }
    }
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

// ── Link classification (seeding suggestions) ─────────────────────────────────

/// Hosts that are job boards / aggregators / employer ATS — never a personal
/// contact link, so they must not seed LinkedIn / GitHub / Website.
const JOB_BOARD_HOSTS: &[&str] = &[
    "indeed.com",
    "glassdoor.com",
    "stepstone.de",
    "stepstone.com",
    "monster.com",
    "ziprecruiter.com",
    "lever.co",
    "greenhouse.io",
    "workday.com",
    "myworkdayjobs.com",
    "ashbyhq.com",
    "smartrecruiters.com",
    "recruitee.com",
    "personio.de",
    "arbeitnow.com",
    "xing.com",
];

fn host_of(url: &str) -> Option<String> {
    let lower = url.trim().to_lowercase();
    let no_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let host = no_scheme.split(['/', '?', '#']).next()?;
    Some(host.trim_start_matches("www.").to_string())
}

fn host_is(url: &str, domain: &str) -> bool {
    host_of(url).is_some_and(|h| h == domain || h.ends_with(&format!(".{domain}")))
}

/// A personal LinkedIn profile is `/in/…`. Company (`/company/…`), school
/// (`/school/…`) and job (`/jobs/…`) pages are NOT the candidate's profile — these
/// are exactly the company-link pool that used to leak into the header.
fn is_personal_linkedin(url: &str) -> bool {
    host_is(url, "linkedin.com") && url.to_lowercase().contains("/in/")
}

/// A personal GitHub profile/repo (any github.com URL that isn't an org settings
/// page is acceptable as the candidate's GitHub).
fn is_github(url: &str) -> bool {
    host_is(url, "github.com")
}

/// Personal-site / link-in-bio hosts that belong under "Website".
const WEBSITE_HOSTS: &[&str] = &[
    "solo.to",
    "bio.link",
    "linktr.ee",
    "bento.me",
    "about.me",
    "carrd.co",
];

fn is_job_board(url: &str) -> bool {
    JOB_BOARD_HOSTS.iter().any(|d| host_is(url, d))
}

/// Classify extracted résumé links into a [`ContactProfile`] by NAME, not by
/// position. Picks the first personal LinkedIn (`/in/`), the first GitHub, and a
/// personal website (a known link-in-bio host, else the first non-job-board,
/// non-platform `http(s)` link). Every remaining personal `http(s)` link (e.g.
/// Dribbble, Behance, a portfolio) is kept as a labelled [`ContactLink`] in
/// `extra_links`, so the header is seeded with the candidate's full link set —
/// never a job-board / employer page. This is a suggestion to seed the editable
/// profile, never the final header on its own.
pub fn classify_contact_links(links: &[Link]) -> ContactProfile {
    let mut profile = ContactProfile::default();
    for link in links {
        let url = link.url.trim();
        if url.is_empty() {
            continue;
        }
        if let Some(email) = url.strip_prefix("mailto:") {
            profile.email.get_or_insert_with(|| email.to_string());
            continue;
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            continue;
        }
        if profile.linkedin.is_none() && is_personal_linkedin(url) {
            profile.linkedin = Some(url.to_string());
            continue;
        }
        if profile.github.is_none() && is_github(url) {
            profile.github = Some(url.to_string());
            continue;
        }
        if profile.website.is_none() && WEBSITE_HOSTS.iter().any(|d| host_is(url, d)) {
            profile.website = Some(url.to_string());
            continue;
        }
    }
    // Website fallback: the first non-job-board, non-platform http(s) link, so a
    // personal portfolio on an arbitrary domain is still surfaced — but an
    // employer / company URL never is.
    if profile.website.is_none() {
        for link in links {
            let url = link.url.trim();
            if (url.starts_with("http://") || url.starts_with("https://"))
                && !is_job_board(url)
                && !host_is(url, "linkedin.com")
                && !is_github(url)
            {
                profile.website = Some(url.to_string());
                break;
            }
        }
    }
    // Extras: every other personal http(s) link, labelled by domain (Dribbble,
    // Behance, …). Skips job boards and the links already promoted to a named
    // field, and de-dupes by URL so the same link is never listed twice.
    let named: std::collections::BTreeSet<&str> = [
        profile.linkedin.as_deref(),
        profile.github.as_deref(),
        profile.website.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    for link in links {
        let url = link.url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://"))
            || is_job_board(url)
            || named.contains(url)
            || profile.extra_links.iter().any(|e| e.url == url)
        {
            continue;
        }
        profile.extra_links.push(ContactLink {
            label: url_label(url),
            url: url.to_string(),
        });
    }
    profile
}

// ── Store (single-row SQLite settings table) ──────────────────────────────────

pub struct ContactProfileStore {
    conn: Mutex<Connection>,
}

impl ContactProfileStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_contact_profile",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS contact_profile (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    data TEXT
                );",
            )?;
            conn.execute("INSERT OR IGNORE INTO contact_profile (id) VALUES (1)", [])?;
            Ok(())
        },
    }];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("contact_profile.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        run_migrations(&conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self) -> ContactProfile {
        let conn = self.conn.lock();
        conn.query_row("SELECT data FROM contact_profile WHERE id = 1", [], |row| {
            let json: Option<String> = row.get(0)?;
            Ok(json)
        })
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    }

    pub fn set(&self, profile: &ContactProfile) -> AppResult<()> {
        let json = serde_json::to_string(profile).map_err(|e| e.to_string())?;
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE contact_profile SET data = ?1 WHERE id = 1",
            rusqlite::params![json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Reset the contact profile to empty (factory reset).
    pub fn clear(&self) -> AppResult<()> {
        self.set(&ContactProfile::default())
    }
}

impl DataStore for ContactProfileStore {
    fn key(&self) -> &'static str {
        "contactProfile"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::to_value(self.get()).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        if data.is_null() {
            return Ok(0);
        }
        let profile: ContactProfile =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        self.set(&profile)?;
        Ok(1)
    }
}

#[cfg(test)]
mod test;
