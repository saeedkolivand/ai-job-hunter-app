//! Pure company/title matching — normalized token-Jaccard scoring of an
//! email's extracted [`crate::email_watch::parser::Candidates`] against the
//! user's ELIGIBLE applications. No IMAP/parser/Tauri coupling: everything
//! here is deterministic and network-free, so it is fixture-tested directly.
//!
//! Fuzzy matching can't hit Layer A's exact-URL bar, so this is deliberately
//! conservative: the company overlap must clear [`COMPANY_THRESHOLD`] on its
//! own (the domain hint and title overlap only ever nudge a borderline score,
//! never substitute for one), and a genuine tie between two eligible
//! applications is treated as ambiguous (`None`) rather than guessed.
//!
//! **Candidacy is NOT "status == Saved" any more.** It is
//! [`crate::email_watch::intent::is_actionable`] — live, OR terminal but
//! itself an unconfirmed email-derived write — the SAME predicate
//! `intent::next_status` uses to decide whether a status may move at all.
//! Narrowing candidacy back to `Saved`-only silently made the whole rest of
//! the status ladder unreachable (a rejection/interview/offer for an
//! `Applied` application could never match), and excluding an
//! unconfirmed-terminal application from candidacy would make `next_status`'s
//! own terminal-override fix dead code one layer up — see `is_actionable`'s
//! doc and this module's `matcher_and_next_status_eligibility_never_disagree`
//! property test, which pins that the two can never drift apart.
//!
//! This module still decides ONLY **which application** — never **what
//! happened**. It takes no [`crate::email_watch::intent::EmailIntent`] and
//! never will; `unconfirmed_email_write_ids` is provenance about the
//! application's OWN current status, computed by the caller (which has the
//! DB access this pure module deliberately does not), not about any
//! particular email.

use std::collections::HashSet;

use crate::applications::Application;
use crate::email_watch::intent::is_actionable;
use crate::email_watch::parser::Candidates;

/// Company-token Jaccard must clear this to be considered at all. Chosen so
/// two genuinely different company names (near-zero overlap) can never pass
/// even with both boosts below maxed out (`DOMAIN_HINT_BOOST +
/// TITLE_BOOST_WEIGHT` is well under this bar on its own).
const COMPANY_THRESHOLD: f64 = 0.5;

/// Small nudge applied when the sender's domain is a known-ATS hint — see
/// [`crate::email_watch::parser::Fingerprint::domain_hint`]'s doc for why
/// this can never gate on its own.
const DOMAIN_HINT_BOOST: f64 = 0.05;

/// Scalar applied to the title-token Jaccard overlap (0.0–1.0) before adding
/// it in — a perfect title match contributes at most this much.
const TITLE_BOOST_WEIGHT: f64 = 0.1;

/// Legal-entity/generic-noise tokens dropped before comparing, so "Acme
/// Corp"/"Acme, Inc."/"Acme GmbH" all normalize to the same token set as
/// plain "Acme".
const STOPWORDS: &[&str] = &[
    "inc",
    "llc",
    "gmbh",
    "corp",
    "corporation",
    "ltd",
    "limited",
    "co",
    "company",
    "the",
    "and",
    "und",
    "ag",
    "kg",
    "se",
];

fn normalize_tokens(s: &str) -> HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && !STOPWORDS.contains(t))
        .map(str::to_string)
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// One saved application scored against a set of [`Candidates`].
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub application_id: String,
    pub score: f64,
}

/// Best-or-none match: the single ELIGIBLE application (see this module's
/// own doc on candidacy) whose company clears [`COMPANY_THRESHOLD`] (after
/// the hint/title nudges) with the strictly HIGHEST score, or `None` if
/// nothing clears the bar, or if the top two scores are exactly tied
/// (ambiguous — never guess between two equally likely candidates).
///
/// `unconfirmed_email_write_ids` — ids for which [`crate::applications::
/// ApplicationStore::current_status_is_unconfirmed_email_write`] is `true`,
/// computed by the caller. Only matters for a TERMINAL application (a live
/// one is always eligible regardless); harmless to include a live
/// application's id too; see [`is_actionable`].
pub fn best_match(
    candidates: &Candidates,
    applications: &[Application],
    domain_hint: bool,
    unconfirmed_email_write_ids: &HashSet<String>,
) -> Option<Scored> {
    let company = candidates.company.as_deref()?;
    let company_tokens = normalize_tokens(company);
    if company_tokens.is_empty() {
        return None;
    }
    let title_tokens = candidates.title.as_deref().map(normalize_tokens);

    let mut ranked: Vec<Scored> = applications
        .iter()
        .filter(|app| is_actionable(app.status, unconfirmed_email_write_ids.contains(&app.id)))
        .filter_map(|app| {
            let app_company_tokens = normalize_tokens(&app.company);
            if app_company_tokens.is_empty() {
                return None;
            }
            let mut score = jaccard(&company_tokens, &app_company_tokens);
            if score <= 0.0 {
                return None; // no company overlap at all — never worth ranking
            }
            if domain_hint {
                score += DOMAIN_HINT_BOOST;
            }
            if let Some(title_tokens) = &title_tokens {
                let app_title_tokens = normalize_tokens(&app.title);
                if !app_title_tokens.is_empty() {
                    score += jaccard(title_tokens, &app_title_tokens) * TITLE_BOOST_WEIGHT;
                }
            }
            (score >= COMPANY_THRESHOLD).then_some(Scored {
                application_id: app.id.clone(),
                score,
            })
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    match ranked.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        [top, second, ..] if (top.score - second.score).abs() < f64::EPSILON => None,
        [top, ..] => Some(top.clone()),
    }
}

#[cfg(test)]
mod tests;
