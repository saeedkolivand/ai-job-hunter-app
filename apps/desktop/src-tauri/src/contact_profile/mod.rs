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

/// A single identity field where the imported résumé's value CONFLICTS with the
/// value already saved in the contact profile (both non-empty, normalized values
/// differ). The import never blocks on these — it still silently fills empty
/// fields — but the renderer surfaces them so the user can resolve each one. The
/// values reported are the ORIGINAL (un-normalized) strings so the UI shows them
/// faithfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactFieldConflict {
    /// Stable key: `email`, `phone`, `linkedin`, `github`, `website`, or
    /// `location`.
    pub field: String,
    /// The value currently saved in the profile (un-normalized).
    pub current: String,
    /// The value extracted from the imported résumé (un-normalized).
    pub suggested: String,
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
    /// DOCX paths — there is no other place a header URL is chosen. Every part
    /// is scheme-checked (link fields only; `javascript:`/`data:` never render as
    /// a clickable header link) and sanitized (control characters incl. `\n`
    /// stripped, length capped) before joining — this string is spliced verbatim
    /// into plain, `\n`-split document text (H's header-seeding path), so an
    /// embedded newline would otherwise inject an arbitrary extra line.
    ///
    /// Sanitization/capping runs on each BARE value (url/label/text) BEFORE a
    /// link field is formatted into `[Label](url)`, not on the formatted
    /// string afterward — capping the formatted string instead can truncate
    /// away the closing `)` for a long-but-legitimate URL, producing a
    /// malformed link that [`Self::header_urls`] (bare-URL-only) would never
    /// reproduce, so the genuinely-rendered link fails set membership there
    /// and false-fires `header_url_mismatch`. Capping the bare URL first, the
    /// same way in both methods, is what keeps them recording the identical
    /// post-cap string by construction. Every label/URL that ends up INSIDE
    /// a `[Label](url)` construct goes through [`sanitize_link_part`], not
    /// plain [`sanitize_header_part`] — it additionally drops `[`, `]`, `(`,
    /// `)` so a value can't close the link early or open a second one;
    /// location/email/phone stay on [`sanitize_header_part`] since they're
    /// never bracket-wrapped.
    pub fn header_markdown(&self, lang: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(loc) = &self.location {
            let v = loc.resolve(lang);
            if !v.trim().is_empty() {
                parts.push(sanitize_header_part(v));
            }
        }
        if let Some(email) = non_empty(&self.email) {
            parts.push(sanitize_header_part(email));
        }
        if let Some(phone) = non_empty(&self.phone) {
            parts.push(sanitize_header_part(phone));
        }
        if let Some(url) = non_empty(&self.linkedin).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[LinkedIn]({})", sanitize_link_part(url)));
        }
        if let Some(url) = non_empty(&self.github).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[GitHub]({})", sanitize_link_part(url)));
        }
        if let Some(url) = non_empty(&self.website).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[Website]({})", sanitize_link_part(url)));
        }
        for link in &self.extra_links {
            let (label, url) = (link.label.trim(), link.url.trim());
            if !label.is_empty() && !url.is_empty() && is_safe_header_url(url) {
                parts.push(format!(
                    "[{}]({})",
                    sanitize_link_part(label),
                    sanitize_link_part(url)
                ));
            }
        }
        parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" | ")
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

    /// Fill in a document header's name / contact line from this profile,
    /// localized for `lang` — a **fallback only**. The editor's text is the source
    /// of truth: a header that already carries a name or a contact line (parsed
    /// from the document text) is left untouched; the profile fills in whichever
    /// of the two the text-derived header is missing. No-op on both fields when
    /// the profile itself has nothing to contribute.
    ///
    /// Name fallback: when `header.name` is blank (e.g. export without generation
    /// metadata that normally fills it), `full_name` from this profile is used so
    /// a profile-edited name is never silently dropped in the rendered output.
    pub fn apply_to_header(&self, header: &mut crate::model::document::HeaderBlock, lang: &str) {
        // Fill the name from the profile when the header carries no name yet.
        // Sanitized like every other field `header_markdown` renders — a
        // control character in `full_name` is otherwise the one field that
        // reaches the header unsanitized (`contact_profile_set` accepts
        // arbitrary JSON behind a bare `z.string()`, so this is not merely a
        // browser-input-behaviour guarantee).
        if header.name.trim().is_empty() {
            if let Some(name) = non_empty(&self.full_name) {
                header.name = sanitize_header_part(name);
            }
        }

        // Fill the contact line from the profile only when the text-derived
        // header carries none — the text (what the editor shows) wins whenever
        // it already has a contact line.
        if header.contact.is_empty() {
            let rich = self.header_rich(lang);
            if !rich.is_empty() {
                header.contact = rich;
            }
        }
    }

    /// The set of header URLs this profile would render (for validation parity
    /// checks across documents — the sole input to
    /// `validate::pdf_render_issues`'s `allowed` set). Email is included as a
    /// `mailto:` link.
    ///
    /// Routed through the SAME `is_safe_header_url` filter [`Self::header_markdown`]
    /// applies, and the same PER-VALUE sanitizer — `sanitize_link_part` for a
    /// URL that renders inside `[Label](…)` there, `sanitize_header_part` for
    /// the bare email — so the two can never fall out of lockstep: an
    /// unsafe-scheme URL `header_markdown` drops must never appear here as
    /// "the profile's own link" (a phantom entry that would otherwise cause a
    /// spurious, non-blocking `header_url_missing`), and a URL/email carrying
    /// a control character, a link-breaking bracket, or exceeding the length
    /// cap must be compared here in the SAME post-sanitize form it actually
    /// renders in — capping the WRAPPED string instead (`mailto:{email}`,
    /// `[Label](url)`) can cap at a different point than the bare-value cap
    /// `header_markdown` applies, so the genuinely-rendered link fails set
    /// membership and `header_url_mismatch` (CRITICAL, blocking) fires on an
    /// unmodified, legitimate profile.
    pub fn header_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(email) = non_empty(&self.email) {
            out.push(format!("mailto:{}", sanitize_header_part(email)));
        }
        for url in [
            non_empty(&self.linkedin),
            non_empty(&self.github),
            non_empty(&self.website),
        ]
        .into_iter()
        .flatten()
        .filter(|u| is_safe_header_url(u))
        {
            out.push(sanitize_link_part(url));
        }
        for link in &self.extra_links {
            let url = link.url.trim();
            if !url.is_empty() && is_safe_header_url(url) {
                out.push(sanitize_link_part(url));
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

/// Scheme allowlist for a header link — `http(s)` or `mailto:` only.
/// `javascript:`/`data:` (and anything else) never render as a clickable
/// header link, however lenient upstream URL classification/import is.
fn is_safe_header_url(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Strip control characters (newlines above all) and cap length on one
/// already-formatted `header_markdown` part. A raw `\n` in a profile field
/// would otherwise inject an arbitrary extra physical line — including a
/// well-formed section heading — once the header line is spliced into plain,
/// `\n`-split document text.
fn sanitize_header_part(s: &str) -> String {
    const MAX_LEN: usize = 200;
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_LEN)
        .collect()
}

/// [`sanitize_header_part`], plus drops `[`, `]`, `(`, `)` — for a label or
/// URL value that gets spliced into a `[Label](url)` markdown construct
/// (never for a bare value like location/email/phone, which isn't bracket-
/// wrapped). Those four characters could otherwise close the link early or
/// open a second one; [`is_safe_header_url`] only checks the scheme prefix,
/// so an `https://`-prefixed value can still carry one past it. A prior
/// security round judged the live exploit surface already closed
/// (import-derived labels come from [`url_label`], which cannot produce a
/// bracket) — this is defense-in-depth, not a hole being patched, but it's
/// cheap and it keeps every label/URL that reaches a `[Label](url)`
/// construct byte-identical between [`ContactProfile::header_markdown`] and
/// [`ContactProfile::header_urls`].
fn sanitize_link_part(s: &str) -> String {
    const MAX_LEN: usize = 200;
    s.chars()
        .filter(|c| !c.is_control() && !matches!(c, '[' | ']' | '(' | ')'))
        .take(MAX_LEN)
        .collect()
}

// ── Conflict detection (resolvable mismatches on import) ──────────────────────

/// Normalize an email for comparison: trim + lowercase.
fn norm_email(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Normalize a phone for comparison: digits only (drops spaces, `()`, `-`, `+`,
/// `.`, and any other formatting), so `+1 (555) 123-4567` == `15551234567`.
fn norm_phone(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// Normalize a URL for comparison: lowercase host (via [`host_of`], which already
/// strips scheme + `www.`) joined with the trimmed path (lowercased, trailing
/// slash removed). Keeps `http` vs `https`, a trailing slash, and a `www.` prefix
/// from registering as conflicts, e.g. `linkedin.com/in/x`,
/// `https://www.linkedin.com/in/x/`, and `http://linkedin.com/in/x` all normalize
/// equal.
fn norm_url(s: &str) -> String {
    let host = host_of(s).unwrap_or_default();
    let lower = s.trim().to_lowercase();
    let no_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let path = no_scheme
        .split(['?', '#'])
        .next()
        .unwrap_or(no_scheme)
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or("");
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        host
    } else {
        format!("{host}/{path}")
    }
}

/// Normalize a plain text value (full name / location) for comparison: trim +
/// lowercase (case-insensitive).
fn norm_text(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Detect the identity fields where `current` (the saved profile) and `suggested`
/// (the imported résumé's extracted contact) CONFLICT — i.e. BOTH are non-empty
/// after trimming AND their normalized values differ. Compares only the
/// single-valued identity fields (email, phone, linkedin, github, website, and
/// `location.default`); `extra_links` is a list merged by URL and is
/// never reported here. The conflict carries the ORIGINAL values so the UI shows
/// them as written. Conservative by design: scheme, trailing slash, and `www.`
/// differences on URLs are NOT conflicts (see [`norm_url`]).
pub fn detect_contact_conflicts(
    current: &ContactProfile,
    suggested: &ContactProfile,
) -> Vec<ContactFieldConflict> {
    let mut out = Vec::new();

    fn push_if_conflict(
        out: &mut Vec<ContactFieldConflict>,
        field: &str,
        cur: Option<&str>,
        sug: Option<&str>,
        normalize: impl Fn(&str) -> String,
    ) {
        if let (Some(cur), Some(sug)) = (cur, sug) {
            if normalize(cur) != normalize(sug) {
                out.push(ContactFieldConflict {
                    field: field.to_string(),
                    current: cur.to_string(),
                    suggested: sug.to_string(),
                });
            }
        }
    }

    push_if_conflict(
        &mut out,
        "email",
        non_empty(&current.email),
        non_empty(&suggested.email),
        norm_email,
    );
    push_if_conflict(
        &mut out,
        "phone",
        non_empty(&current.phone),
        non_empty(&suggested.phone),
        norm_phone,
    );
    push_if_conflict(
        &mut out,
        "linkedin",
        non_empty(&current.linkedin),
        non_empty(&suggested.linkedin),
        norm_url,
    );
    push_if_conflict(
        &mut out,
        "github",
        non_empty(&current.github),
        non_empty(&suggested.github),
        norm_url,
    );
    push_if_conflict(
        &mut out,
        "website",
        non_empty(&current.website),
        non_empty(&suggested.website),
        norm_url,
    );

    let cur_loc = current
        .location
        .as_ref()
        .map(|l| l.default.trim())
        .filter(|s| !s.is_empty());
    let sug_loc = suggested
        .location
        .as_ref()
        .map(|l| l.default.trim())
        .filter(|s| !s.is_empty());
    push_if_conflict(&mut out, "location", cur_loc, sug_loc, norm_text);

    out
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

/// A github.com URL. Combined with `is_profile_shaped` at the call site so only
/// `github.com/<user>` (not `/<user>/<repo>`) is promoted to the candidate's
/// GitHub — a repo link is a project reference, not an identity.
fn is_github(url: &str) -> bool {
    host_is(url, "github.com")
}

/// Known social/portfolio platform hosts whose profile page belongs on the
/// contact line. Mirrors `PROFILE_DOMAINS` in
/// `packages/prompts/src/generate/links/links.ts` — keep the two lists in sync.
const PROFILE_HOSTS: &[&str] = &[
    "linkedin.com",
    "github.com",
    "gitlab.com",
    "twitter.com",
    "x.com",
    "behance.net",
    "dribbble.com",
    "medium.com",
    "stackoverflow.com",
    "dev.to",
    "codepen.io",
    "youtube.com",
    "youtu.be",
    "notion.so",
    "figma.com",
    "npmjs.com",
    "crates.io",
    "solo.to",
    "bio.link",
    "linktr.ee",
    "bento.me",
];

fn is_profile_host(url: &str) -> bool {
    PROFILE_HOSTS.iter().any(|d| host_is(url, d))
}

/// Non-empty path segments of `url` (host, query and fragment stripped).
/// Mirrors `pathSegments()` in links.ts (only the *count* matters here, so
/// unlike the TS version this does not percent-decode).
fn path_segments(url: &str) -> Vec<&str> {
    let trimmed = url.trim();
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let path = match no_scheme.find('/') {
        Some(idx) => no_scheme[idx..].split(['?', '#']).next().unwrap_or(""),
        None => "",
    };
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// A bare-root URL — host only, no meaningful path. The shape of a homepage.
/// Mirrors `isBareRoot()` in links.ts.
fn is_bare_root(url: &str) -> bool {
    path_segments(url).is_empty()
}

/// Is this platform URL a *profile* (belongs on the contact line) rather than a
/// deep link to a specific repo/article (a project reference, which belongs in
/// the résumé body, not the header)? `github.com/<user>` is a profile;
/// `github.com/<user>/<repo>` is a project. Mirrors `isProfileShaped()` in
/// links.ts.
fn is_profile_shaped(url: &str) -> bool {
    if host_is(url, "github.com") || host_is(url, "gitlab.com") {
        return path_segments(url).len() <= 1;
    }
    true
}

/// A link shaped like a **platform profile** — the only kind of link that may
/// seed `extra_links`: a profile-shaped host from [`PROFILE_HOSTS`] (GitHub,
/// Dribbble, Behance, …), or a personal LinkedIn (`/in/`) profile. LinkedIn
/// keeps the stricter `is_personal_linkedin` gate instead of the generic
/// `is_profile_shaped` rule — a company/school page is otherwise
/// indistinguishable by shape but must never seed the header. A bare-root
/// *personal* domain (no known platform) is deliberately excluded here — at
/// most one such domain is ever admitted to the profile at all, as `website`
/// (see the fallback in [`classify_contact_links`]); every other one is a
/// body/project link and must never re-enter the profile. Shared rule with
/// `isProfileUrl`/`isProfileShaped`/`classifyLinks` in
/// `packages/prompts/src/generate/links/links.ts`: a platform-profile URL
/// stays on the contact side; a non-platform URL is admitted at most once
/// (`Website`) — every other one is a body link.
fn is_platform_profile_link(url: &str) -> bool {
    if host_is(url, "linkedin.com") {
        return is_personal_linkedin(url);
    }
    is_profile_host(url) && is_profile_shaped(url)
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

/// Public for `validate::pdf_render_issues` — a header-band link is still
/// worth a warning when it resolves to a known job-board/ATS host, even on the
/// text-derived-header path where the profile isn't the source of truth.
pub(crate) fn is_job_board(url: &str) -> bool {
    JOB_BOARD_HOSTS.iter().any(|d| host_is(url, d))
}

/// Classify extracted résumé links into a [`ContactProfile`] by NAME and SHAPE,
/// not by position. Picks the first personal LinkedIn (`/in/`), the first
/// profile-shaped GitHub (`github.com/<user>`, never `/<user>/<repo>`), and a
/// personal website (a known link-in-bio host, else a bare-root, non-platform
/// `http(s)` link — an apex host wins over any candidate that is one of its
/// own subdomains, then first-seen decides; this is order-independent). Every
/// remaining *platform-profile* link — a profile-shaped platform profile
/// (Dribbble, Behance, a second GitHub user) — is kept as a labelled
/// [`ContactLink`] in `extra_links`. A bare-root personal domain that was not
/// promoted to `website`, and any deep-path project/demo/article/repo link,
/// never enters the profile at all — it belongs in the résumé body, not the
/// header. Shared rule with `classifyLinks` in
/// `packages/prompts/src/generate/links/links.ts`: a platform-profile URL
/// stays on the contact side (LinkedIn gated to `/in/`, same as
/// `is_personal_linkedin` here); a non-platform URL is admitted at most once,
/// as `Website`, via the same order-independent apex-over-subdomain
/// preference (`pickWebsiteUrl` there mirrors `is_subdomain_of_another` /
/// `is_apex_of_another` here, dot-prefixed suffix check included) — every
/// other bare-root candidate is a body/project link.
///
/// This is a suggestion to seed the editable profile, never the final header on
/// its own.
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
        if profile.github.is_none() && is_github(url) && is_profile_shaped(url) {
            profile.github = Some(url.to_string());
            continue;
        }
        if profile.website.is_none() && WEBSITE_HOSTS.iter().any(|d| host_is(url, d)) {
            profile.website = Some(url.to_string());
            continue;
        }
    }
    // Website fallback: a non-job-board, non-platform, bare-root http(s) link,
    // so a personal portfolio homepage is still surfaced — but an
    // employer/company URL or a deep link (e.g. a project demo path) never is.
    // `!is_profile_host` subsumes the old explicit linkedin/github checks.
    //
    // Which candidate wins is order-independent: an apex host (one that is
    // itself the parent of another candidate host in this same document, e.g.
    // `apex.dev` alongside `sub.apex.dev`) is preferred over every standalone
    // candidate, because the apex/subdomain relationship is stronger, shape-
    // based evidence of "the" personal domain than raw document position.
    // Among hosts tied on that signal, first-seen decides.
    if profile.website.is_none() {
        let candidates: Vec<(String, &str)> = links
            .iter()
            .filter_map(|link| {
                let url = link.url.trim();
                let is_candidate = (url.starts_with("http://") || url.starts_with("https://"))
                    && !is_job_board(url)
                    && !is_profile_host(url)
                    && is_bare_root(url);
                if !is_candidate {
                    return None;
                }
                host_of(url).map(|h| (h, url))
            })
            .collect();
        let hosts: Vec<&str> = candidates.iter().map(|(h, _)| h.as_str()).collect();
        let is_subdomain_of_another = |host: &str| {
            hosts
                .iter()
                .any(|o| *o != host && host.ends_with(&format!(".{o}")))
        };
        let is_apex_of_another = |host: &str| {
            hosts
                .iter()
                .any(|o| *o != host && o.ends_with(&format!(".{host}")))
        };
        let apex_pick = candidates
            .iter()
            .find(|(h, _)| !is_subdomain_of_another(h) && is_apex_of_another(h));
        let first_pick = candidates.iter().find(|(h, _)| !is_subdomain_of_another(h));
        if let Some((_, url)) = apex_pick.or(first_pick) {
            profile.website = Some(url.to_string());
        }
    }
    // Extras: every other platform-profile http(s) link, labelled by domain
    // (Dribbble, Behance, a second GitHub user, …). A bare-root personal
    // domain that lost the `website` slot above is NOT an extra — it never
    // re-enters the profile. Skips job boards and the links already promoted
    // to a named field, and de-dupes by URL so the same link is never listed
    // twice.
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
            || !is_platform_profile_link(url)
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
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("contact_profile.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
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
