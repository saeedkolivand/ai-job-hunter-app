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
/// platforms (career-ops's original list) plus the hosts our own 21
/// `SCRAPERS` boards legitimately return as `JobPosting.url` where that host
/// is the BOARD's own domain rather than the employer's — LinkedIn
/// (`linkedin.com`, always `/jobs/view/<id>`), Berlin Startup Jobs
/// (`berlinstartupjobs.com`, its own WordPress RSS permalink), and the Adzuna
/// aggregator (`api.adzuna.com` — the country code is a *path* segment, e.g.
/// `/v1/api/jobs/de/redirects/…`, not a subdomain, so this one host covers
/// every market's `redirect_url`) — so those boards' results aren't
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

    if !company.trim().is_empty()
        && !matches_domain_list(&host, ATS_ALLOWLIST)
        && !company_matches_host(company, &host)
    {
        flags.push(TrustFlag::CompanyDomainMismatch);
        score -= 15;
    }

    finish(score, flags)
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
const COMPANY_NAME_STOP_WORDS: &[&str] = &["the", "inc", "llc", "ltd", "corp", "gmbh", "co"];

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
