//! Posting identity verdict (issue #1445): is a freshly resolved posting still
//! the SAME advert a stored row describes, or did the url now lead somewhere
//! else (an aggregator redirect that moved to another job, a re-used url)?
//! Pure, no IO.
//!
//! Two tiers, both reusing existing identity knowledge rather than a new matcher:
//! 1. When both urls carry a `(board, id)` identity on the SAME board
//!    ([`super::job_identity`], issue #1166) the id alone decides — regional
//!    and slug variants of one posting stay [`IdentityVerdict::Same`].
//! 2. Otherwise (a cross-board redirect such as Adzuna → Greenhouse, or a board
//!    with no stable id) company and title are compared with the SAME
//!    normalization + thresholds the cross-board clustering pass uses
//!    (ADR-029 `scraping::cluster`): equal normalized company, equal title
//!    first token, trigram-Jaccard of the normalized titles above the cluster
//!    floor. A blank value on either side is "no evidence", not a mismatch.

use crate::scraping::cluster::normalize::{normalize_company, normalize_title, title_first_token};
use crate::scraping::cluster::{trigram_jaccard, CLUSTER_TITLE_TRIGRAM_JACCARD_MIN};

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
}

fn company_matches(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize_company(a), normalize_company(b));
    na.is_empty() || nb.is_empty() || na == nb
}

fn title_matches(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize_title(a), normalize_title(b));
    na.is_empty()
        || nb.is_empty()
        || na == nb
        || (title_first_token(a) == title_first_token(b)
            && trigram_jaccard(&na, &nb) >= CLUSTER_TITLE_TRIGRAM_JACCARD_MIN)
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
    if !company_matches(stored.company, resolved.company) {
        fields.push(DriftField::Company);
    }
    if !title_matches(stored.title, resolved.title) {
        fields.push(DriftField::Title);
    }
    if fields.is_empty() {
        IdentityVerdict::Same
    } else {
        IdentityVerdict::Drift(fields)
    }
}

/// Stable fingerprint of a resolved identity, stored when the user dismisses a
/// drift so the SAME drift does not re-flag on the next refresh (a different
/// one does).
pub fn identity_fingerprint(resolved: PostingIdentity<'_>) -> String {
    format!(
        "{}\u{1}{}\u{1}{}",
        crate::applications::normalize_job_url(resolved.url),
        normalize_company(resolved.company),
        normalize_title(resolved.title)
    )
}

#[cfg(test)]
mod tests;
