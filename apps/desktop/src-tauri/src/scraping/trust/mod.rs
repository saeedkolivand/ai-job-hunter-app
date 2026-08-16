//! Job trust / ghost-job signal — a pure, non-blocking enrichment computed for
//! every scraped posting so the renderer can badge suspicious listings.
//!
//! Ported from santifer/career-ops's `providers/_trust-validator.mjs`
//! (MIT License) — <https://github.com/santifer/career-ops>.
//!
//! V1 is flag-only: **enrich, never drop**. A low score never removes a
//! posting; it only lowers [`TrustAssessment::level`] for the UI badge (a
//! separate frontend pass). No config/enabled toggle — always computed.

use super::types::JobPosting;
use serde::{Deserialize, Serialize};

/// Result of [`assess_trust`] — attached to every finalized [`JobPosting`] via
/// [`attach`], never left unset. Also stored (as `Option`) on Autopilot's
/// persisted `FoundJob` record, so this derives `Deserialize` too — a
/// pre-PR3 `FoundJob` on disk has no `trust` key and deserializes `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustAssessment {
    pub score: u8,
    pub level: TrustLevel,
    pub flags: Vec<TrustFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustFlag {
    MissingApplyUrl,
    InvalidUrl,
    SuspiciousDomain,
    CompanyDomainMismatch,
    /// The "company" field itself is implausible as an employer name — CTA/UI
    /// debris (`"Apply now"`), a job board's own brand standing in for the
    /// employer, separator/markup debris, a placeholder, or an unreasonable
    /// shape. See [`is_implausible_company`]. Additive variant — see that
    /// function's doc comment for why appending it here is safe for every
    /// `TrustAssessment`/`FoundJob` record already on disk.
    ImplausibleCompany,
}

/// URL-shortener domains — they obscure the real destination, a classic
/// ghost-job / phishing tell. Suffix-matched via [`matches_domain_list`].
const SUSPICIOUS_DOMAINS: &[&str] = &[
    "bit.ly",
    "tinyurl.com",
    "t.co",
    "forms.gle",
    "goo.gl",
    "shorturl.at",
    "rebrand.ly",
    "cutt.ly",
];

/// Hosts a `CompanyDomainMismatch` is never raised against: real ATS
/// platforms (career-ops's original list) plus the hosts our
/// `SCRAPERS` boards legitimately return as `JobPosting.url` where that host
/// is the BOARD's own domain rather than the employer's — LinkedIn
/// (`linkedin.com`, always `/jobs/view/<id>`), Berlin Startup Jobs
/// (`berlinstartupjobs.com`, its own WordPress RSS permalink), and the Adzuna
/// aggregator (`api.adzuna.com` — the country code is a *path* segment, e.g.
/// `/v1/api/jobs/de/redirects/…`, not a subdomain, so this one host covers
/// every market's `redirect_url`), and Jobicy (`jobicy.com` — its own posting
/// page URL is REQUIRED by Jobicy's ToS attribution, see
/// `scraping/boards/jobicy/mod.rs`) — so those boards' results aren't
/// systematically flagged. JSearch's `job_apply_link` is the real employer
/// URL, so it's intentionally left off this list.
const ATS_ALLOWLIST: &[&str] = &[
    "greenhouse.io",
    "boards.greenhouse.io",
    "ashbyhq.com",
    "lever.co",
    "workday.com",
    "myworkdayjobs.com",
    "smartrecruiters.com",
    "recruitee.com",
    "workable.com",
    "apply.workable.com",
    "icims.com",
    "taleo.net",
    "applytojob.com",
    "breezy.hr",
    "bamboohr.com",
    "pinpointhq.com",
    "rippling.com",
    "ats.rippling.com",
    "personio.de",
    "jobs.personio.de",
    "teamtailor.com",
    "themuse.com",
    "remoteok.com",
    "remotive.com",
    "weworkremotely.com",
    "arbeitnow.com",
    "linkedin.com",
    "berlinstartupjobs.com",
    "api.adzuna.com",
    "comeet.co",
    "jobicy.com",
];

/// Score/flag a posting from its apply `url` and `company` name. Pure, no I/O;
/// never panics on untrusted input.
pub fn assess_trust(url: &str, company: &str) -> TrustAssessment {
    let mut flags = Vec::new();

    if url.trim().is_empty() {
        flags.push(TrustFlag::MissingApplyUrl);
        return finish(100 - 40, flags);
    }

    let parsed = match reqwest::Url::parse(url) {
        Ok(u) if u.scheme() == "http" || u.scheme() == "https" => u,
        _ => {
            flags.push(TrustFlag::InvalidUrl);
            return finish(100 - 50, flags);
        }
    };

    let host = parsed.host_str().unwrap_or_default().to_lowercase();
    let mut score: i32 = 100;

    if matches_domain_list(&host, SUSPICIOUS_DOMAINS) {
        flags.push(TrustFlag::SuspiciousDomain);
        score -= 25;
    }

    if !company.trim().is_empty() {
        if is_implausible_company(company) {
            // Same root cause `company_matches_host` would also fail on below
            // (garbage text rarely matches any host), so this branch takes
            // priority over `CompanyDomainMismatch` rather than stacking both
            // flags for one underlying problem.
            flags.push(TrustFlag::ImplausibleCompany);
            score -= 20;
        } else if !matches_domain_list(&host, ATS_ALLOWLIST)
            && !company_matches_host(company, &host)
        {
            flags.push(TrustFlag::CompanyDomainMismatch);
            score -= 15;
        }
    }

    finish(score, flags)
}

/// Job-board brands that occasionally leak into the "company" field when a
/// scraper's selector lands on the hosting board's own chrome instead of the
/// employer (a nav link, a "Powered by" footer). Judges the COMPANY STRING
/// itself — distinct from [`ATS_ALLOWLIST`]/[`SUSPICIOUS_DOMAINS`] above,
/// which judge the apply URL's host. Matched against the ENTIRE normalized
/// name, not a substring or an isolated word: a bare `"LinkedIn"` is almost
/// certainly board chrome, but real employers legitimately share a board's
/// name as one word among several — `"Xing SE"`, `"Indeed Inc"`,
/// `"Glassdoor Inc"`, `"Monster Worldwide"` all trade under names that
/// contain a board word, and even a prior word-boundary version of this
/// check (rejecting the isolated word "xing"/"indeed"/… anywhere in the
/// name) false-positived every one of them. A false positive here silently
/// deletes a real employer from a letter — worse than the debris this list
/// exists to catch — so it only fires when the board name IS the entire
/// company field, nothing more.
const JOB_BOARD_NAMES: &[&str] = &[
    "linkedin",
    "indeed",
    "glassdoor",
    "xing",
    "stepstone",
    "monster",
    "ziprecruiter",
];

/// Call-to-action / UI copy a broken selector sometimes captures instead of
/// an employer name, matched against the ENTIRE normalized "company" value
/// — never as a substring. A substring check against `"apply on "`
/// previously false-positived the real employer "Apply On Demand Inc". The
/// board-chrome-appended shape from the literal PR #960 report
/// (`"Apply now | LinkedIn"`) is caught by the separator rule above instead
/// of this list, since concatenating a CTA phrase with board chrome no
/// longer matches any single entry exactly.
const CTA_PHRASES: &[&str] = &["apply now", "view job", "see more", "easy apply"];

/// Literal placeholders/nulls a form default or a scraper's own fallback
/// sometimes writes rather than leaving the field empty. Compared against
/// the fully-trimmed, lower-cased value, never as a substring.
const PLACEHOLDER_NAMES: &[&str] = &["n/a", "none", "unknown", "company", "unternehmen"];

/// Longest legitimate employer name observed in the wild is ~48 chars
/// ("CHECK24 Vergleichsportal für Versicherungen GmbH" — see the identical
/// note on `sanitizeCompanyName`, this predicate's TS twin for AI-extracted
/// metadata, `packages/prompts/src/generate/metadata/metadata.ts`). 80 gives
/// a long-form legal name (umlauts, multiple suffixes) real headroom without
/// accepting an obvious sentence or paragraph a broken selector scraped —
/// the character/punctuation/board-name checks below carry most of the
/// classification weight, this bound only catches the extreme tail.
const MAX_COMPANY_CHARS: usize = 80;

/// True when `s` contains an HTML character entity (`&amp;`, `&#39;`, …): an
/// `&` followed, within a short run of non-whitespace characters, by a `;`.
/// Deliberately narrower than "contains `&`" — a real name may contain a bare
/// ampersand (`"Johnson & Johnson"`, `"AT&T"`), and neither has a `;`
/// anywhere near it, so both survive this check untouched.
fn has_html_entity(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != '&' {
            continue;
        }
        let mut j = i + 1;
        let mut scanned = 0;
        while j < chars.len() && scanned < 10 {
            let next = chars[j];
            if next == ';' {
                return true;
            }
            if next.is_whitespace() || next == '&' {
                break;
            }
            j += 1;
            scanned += 1;
        }
    }
    false
}

/// Reject a "company" value that is obviously CTA/UI debris, a job board's
/// own brand, separator/markup debris, a placeholder, or an implausible
/// shape — rather than a real employer name. Pure, no I/O, never panics.
///
/// Mirrors the intent (and the conservatism) of `sanitizeCompanyName`, the
/// existing TS gate on AI-EXTRACTED metadata
/// (`packages/prompts/src/generate/metadata/metadata.ts`) — this is the
/// Rust-side twin for a value that never goes through that extraction step
/// at all: a scraper's own `company` field, ingested straight from the
/// posting. PR #960's postmortem is the reason this exists: a scraper
/// returned `"Apply now | LinkedIn"` as the company, and the letter
/// faithfully addressed it — every downstream validator compares generated
/// text against the job AD, so a garbage company that IS in the ad (because
/// the ad's own scrape carried the same debris) passes every one of them.
///
/// **Deliberately conservative.** A false positive here silently drops a
/// real company name from a cover letter — worse than the bug this predicate
/// exists to catch — so every rule below rejects a SHAPE a real employer
/// name cannot plausibly have, never merely an unusual one. Real names that
/// must survive: `"Johnson & Johnson"`, `"Ben & Jerry's"`, `"Yahoo!"`,
/// `"Booking.com"`, `"37signals"`, and — the reason [`JOB_BOARD_NAMES`] and
/// [`CTA_PHRASES`] are whole-string matches, not substring/word ones —
/// `"Xing SE"`, `"Indeed Inc"`, `"Glassdoor Inc"`, `"Monster Worldwide"`,
/// `"Apply On Demand Inc"`.
///
/// **`"X"` (a single character) is deliberately rejected too**, even though
/// it is the real, current legal name of the company formerly known as
/// Twitter. A bare single character reaching this predicate is overwhelmingly
/// scraper truncation (a stray initial, a cut-off selector), not a genuine
/// application to X Corp — and the false-positive cost for the rare real
/// case is soft: the letter omits the company line and names only the role,
/// which the existing prompt contract already handles gracefully (no
/// placeholder, no crash — see `packages/prompts/src/generate/cover-letter/
/// cover-letter.ts`). Accepting that one narrow, low-cost trade is what lets
/// this predicate reject single-character garbage everywhere else.
pub fn is_implausible_company(name: &str) -> bool {
    let trimmed = name.trim();
    let chars: Vec<char> = trimmed.chars().collect();

    if chars.is_empty() {
        return true;
    }
    if chars.len() == 1 {
        return true;
    }
    if chars.len() > MAX_COMPANY_CHARS {
        return true;
    }

    // Separator / markup debris a scraper's selector miss commonly leaves
    // behind (breadcrumbs, a "CTA | Board" concatenation, raw HTML).
    if trimmed.contains(['|', '·', '<', '>']) {
        return true;
    }
    if has_html_entity(trimmed) {
        return true;
    }

    let alnum = chars.iter().filter(|c| c.is_alphanumeric()).count();
    if alnum == 0 {
        return true;
    }
    let non_space = chars.iter().filter(|c| !c.is_whitespace()).count();
    // "Mostly punctuation": fewer than half the non-space characters are
    // alphanumeric. The lightest punctuation-bearing real name above,
    // "Ben & Jerry's", is still ~80% alphanumeric — comfortably clear of
    // this floor.
    if alnum * 2 < non_space {
        return true;
    }

    let normalized = trimmed.to_lowercase();

    if PLACEHOLDER_NAMES.contains(&normalized.as_str()) {
        return true;
    }

    if CTA_PHRASES.contains(&normalized.as_str()) {
        return true;
    }

    // Whole-string match, not substring/word — see `JOB_BOARD_NAMES`'s doc.
    JOB_BOARD_NAMES.contains(&normalized.as_str())
}

/// Compute [`assess_trust`] for `job` and attach it as `job.extra["trust"]` —
/// the same board-specific-metadata channel `#[serde(flatten)]` already
/// exposes for e.g. salary, so the shape reaches the renderer without a new
/// dedicated struct field (which would force every board's `JobPosting`
/// literal to populate it). A serialization failure is unreachable for this
/// all-primitive struct, but is tolerated (posting still ships, just without
/// `trust`) rather than risking a panic on the hot scrape path.
pub fn attach(job: &mut JobPosting) {
    let assessment = assess_trust(&job.url, &job.company);
    if let Ok(value) = serde_json::to_value(&assessment) {
        job.extra.insert("trust".to_string(), value);
    }
}

fn finish(score: i32, flags: Vec<TrustFlag>) -> TrustAssessment {
    let score = score.clamp(0, 100) as u8;
    let level = if score >= 90 {
        TrustLevel::High
    } else if score >= 60 {
        TrustLevel::Medium
    } else {
        TrustLevel::Low
    };
    TrustAssessment {
        score,
        level,
        flags,
    }
}

/// Does `host` equal or end with `.{d}` for any domain `d` in `list`?
pub(crate) fn matches_domain_list(host: &str, list: &[&str]) -> bool {
    list.iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Generic legal-entity words that appear in countless unrelated company
/// names — skipped in the per-word fallback below so e.g. "The Inc Corp"
/// doesn't false-match almost any host that happens to contain "the" or
/// "corp" as a substring. Advisory-only check (see the doc comment on
/// [`company_matches_host`]), so a short denylist is enough.
const COMPANY_NAME_STOP_WORDS: &[&str] = &["the", "inc", "llc", "ltd", "corp", "gmbh"];

/// Best-effort "is this posting's host plausibly the company's own domain (or
/// an ATS subdomain naming it)?" check. An unjudgeable (empty-after-normalize)
/// company name returns `true` (no flag) rather than guessing.
///
/// Known heuristic limitation, both directions, from the unanchored
/// `host.contains(..)` match: (a) it **misses** a brand-embedding phishing
/// host — `"Amazon"` vs. `amazon-careers.xyz` matches and suppresses the
/// flag, staying `High` — and (b) a short (≤2-char, post `len >= 3` filter
/// mostly avoids this, but a 3-char word like `"AWS"`) or generic company
/// word can over-match an unrelated host. [`COMPANY_NAME_STOP_WORDS`] closes
/// the most common instance of (b) (generic legal-entity suffixes), but not
/// every generic word. Label-boundary anchoring (matching `company.tld` /
/// `company.` / `.company.` rather than a bare substring) would close (a),
/// but was deliberately **deferred for V1**: it trades that miss for false
/// positives on legitimate brand+suffix domains (e.g. `datadoghq.com` vs.
/// `Datadog`, `getbamboohr.com` vs. `BambooHR`). The resulting flag is
/// advisory/non-gating — it only lowers a badge level, it never hides or
/// drops a posting — so the false-negative is an accepted V1 trade-off.
/// Revisit this anchoring if any future flow ever gates behavior (e.g.
/// auto-hide, auto-skip) on `TrustAssessment::level`.
pub(crate) fn company_matches_host(company: &str, host: &str) -> bool {
    let normalized: String = company
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .collect();
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return true;
    }

    let slug: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    if !slug.is_empty() && host.contains(&slug) {
        return true;
    }

    normalized
        .split_whitespace()
        .filter(|word| !COMPANY_NAME_STOP_WORDS.contains(word))
        .any(|word| word.len() >= 3 && host.contains(word))
}

#[cfg(test)]
mod test;
