//! Posting identity verdict (issue #1445): is a freshly resolved posting still
//! the SAME advert a stored row describes, or did the url now lead somewhere
//! else (an aggregator redirect that moved to another job, a re-used url)?
//! Pure, no IO.
//!
//! Two tiers, both reusing existing identity knowledge rather than a new matcher:
//! 1. When both urls carry a `(board, id)` identity on the SAME board
//!    ([`super::job_identity`], issue #1166) the id alone decides — regional
//!    and slug variants of one posting stay [`IdentityVerdict::Same`]. This
//!    only applies where `job_identity` knows the host (LinkedIn, Indeed).
//! 2. Everywhere else (a cross-board redirect such as Adzuna → Greenhouse, or
//!    any other ATS) company and title decide, using the cluster module's
//!    normalization (ADR-029 `scraping::cluster`) with redirect-friendly
//!    matching: company names are equal or one is a whole-word prefix of the
//!    other; titles are equal, one contains the other, or clear a looser
//!    Jaccard floor than clustering uses. So a same-title requisition at the
//!    same company on another ATS reads as Same — a known limit of having no
//!    stable id there.
//!
//! A blank value on either side is "no evidence", not a mismatch; when nothing
//! at all could be compared the verdict is [`IdentityVerdict::NoEvidence`].

use crate::scraping::cluster::normalize::{normalize_company, normalize_title};
use crate::scraping::cluster::trigram_jaccard;

/// Looser than the clustering floor on purpose: a redirect check only has to
/// tell "same advert, differently worded" from "a different advert", where
/// clustering must never merge two distinct roles.
const REDIRECT_TITLE_JACCARD_MIN: f64 = 0.6;

/// Borrowed `(title, company, url)` view of one posting.
#[derive(Debug, Clone, Copy)]
pub struct PostingIdentity<'a> {
    pub title: &'a str,
    pub company: &'a str,
    pub url: &'a str,
}

/// Which part of the identity changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftField {
    Id,
    Company,
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityVerdict {
    Same,
    Drift(Vec<DriftField>),
    /// Nothing could be compared (resolved title and company blank, no
    /// same-board id): neither a write nor a flag is justified.
    NoEvidence,
}

/// Normalized title tokens with seniority abbreviations folded
/// (`sr`/`snr` → senior, `jr` → junior).
fn title_tokens(s: &str) -> Vec<String> {
    normalize_title(s)
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .map(|t| match t {
            "sr" | "snr" => "senior",
            "jr" => "junior",
            other => other,
        })
        .map(str::to_string)
        .collect()
}

/// Same company when the normalized names are equal or one is a whole-word
/// prefix of the other ("BMW Group" / "BMW AG", "Amazon" / "Amazon Web Services").
fn company_matches(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize_company(a), normalize_company(b));
    let (short, long) = if na.len() <= nb.len() {
        (&na, &nb)
    } else {
        (&nb, &na)
    };
    long == short
        || long
            .strip_prefix(short.as_str())
            .is_some_and(|rest| rest.starts_with(' '))
}

/// Same title when the token sequences are equal, one contains the other
/// (e.g. a trailing team suffix), or they clear the redirect Jaccard floor.
fn title_matches(a: &str, b: &str) -> bool {
    let (ta, tb) = (title_tokens(a), title_tokens(b));
    let (short, long) = if ta.len() <= tb.len() {
        (&ta, &tb)
    } else {
        (&tb, &ta)
    };
    long.windows(short.len()).any(|w| w == short.as_slice())
        || trigram_jaccard(&ta.join(" "), &tb.join(" ")) >= REDIRECT_TITLE_JACCARD_MIN
}

/// Compare a stored posting against a freshly resolved one.
pub fn identity_verdict(
    stored: PostingIdentity<'_>,
    resolved: PostingIdentity<'_>,
) -> IdentityVerdict {
    if let (Some((sb, sid)), Some((rb, rid))) = (
        super::job_identity(stored.url),
        super::job_identity(resolved.url),
    ) {
        if sb == rb {
            return if sid == rid {
                IdentityVerdict::Same
            } else {
                IdentityVerdict::Drift(vec![DriftField::Id])
            };
        }
    }
    let mut fields = Vec::new();
    let mut compared = false;
    if !normalize_company(stored.company).is_empty()
        && !normalize_company(resolved.company).is_empty()
    {
        compared = true;
        if !company_matches(stored.company, resolved.company) {
            fields.push(DriftField::Company);
        }
    }
    if !title_tokens(stored.title).is_empty() && !title_tokens(resolved.title).is_empty() {
        compared = true;
        if !title_matches(stored.title, resolved.title) {
            fields.push(DriftField::Title);
        }
    }
    if !fields.is_empty() {
        IdentityVerdict::Drift(fields)
    } else if compared {
        IdentityVerdict::Same
    } else {
        IdentityVerdict::NoEvidence
    }
}

/// Stable fingerprint of a resolved identity, stored when the user dismisses a
/// drift so the SAME drift does not re-flag on the next refresh (a different
/// one does). Uses the `(board, id)` identity where known, so regional hosts
/// of one posting fingerprint alike.
pub fn identity_fingerprint(resolved: PostingIdentity<'_>) -> String {
    let location = match super::job_identity(resolved.url) {
        Some((board, id)) => format!("{board}:{id}"),
        None => crate::applications::normalize_job_url(resolved.url),
    };
    format!(
        "{location}\u{1}{}\u{1}{}",
        normalize_company(resolved.company),
        normalize_title(resolved.title)
    )
}

#[cfg(test)]
mod tests;
