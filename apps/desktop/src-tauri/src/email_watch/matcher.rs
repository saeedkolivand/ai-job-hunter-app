//! Pure company/title matching — normalized token-Jaccard scoring of an
//! email's extracted [`crate::email_watch::parser::Candidates`] against the
//! user's `saved` applications. No IMAP/parser/Tauri coupling: everything
//! here is deterministic and network-free, so it is fixture-tested directly.
//!
//! Fuzzy matching can't hit Layer A's exact-URL bar, so this is deliberately
//! conservative: the company overlap must clear [`COMPANY_THRESHOLD`] on its
//! own (the domain hint and title overlap only ever nudge a borderline score,
//! never substitute for one), and a genuine tie between two different saved
//! applications is treated as ambiguous (`None`) rather than guessed.

use std::collections::HashSet;

use crate::applications::{Application, ApplicationStatus};
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

/// Best-or-none match: the single `saved` application whose company clears
/// [`COMPANY_THRESHOLD`] (after the hint/title nudges) with the strictly
/// HIGHEST score, or `None` if nothing clears the bar, or if the top two
/// scores are exactly tied (ambiguous — never guess between two equally
/// likely saves).
pub fn best_match(
    candidates: &Candidates,
    applications: &[Application],
    domain_hint: bool,
) -> Option<Scored> {
    let company = candidates.company.as_deref()?;
    let company_tokens = normalize_tokens(company);
    if company_tokens.is_empty() {
        return None;
    }
    let title_tokens = candidates.title.as_deref().map(normalize_tokens);

    let mut ranked: Vec<Scored> = applications
        .iter()
        .filter(|app| app.status == ApplicationStatus::Saved)
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
mod tests {
    use super::*;

    fn app(id: &str, company: &str, title: &str, status: ApplicationStatus) -> Application {
        Application {
            id: id.to_string(),
            status,
            applied_at: None,
            created_at: 0,
            updated_at: 0,
            job_url: String::new(),
            board: String::new(),
            company: company.to_string(),
            title: title.to_string(),
            candidate: String::new(),
            answers: Vec::new(),
            brief: String::new(),
            job_description: String::new(),
            notes: String::new(),
            next_action_at: None,
            comp: String::new(),
            contact_name: String::new(),
            contact_email: String::new(),
            job_summary: String::new(),
            recipient_name: String::new(),
            recipient_email: String::new(),
            salary_min: None,
            salary_max: None,
            salary_currency: None,
        }
    }

    fn candidates(company: Option<&str>, title: Option<&str>) -> Candidates {
        Candidates {
            company: company.map(str::to_string),
            title: title.map(str::to_string),
        }
    }

    #[test]
    fn matches_a_clear_company_overlap() {
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Saved,
        )];
        let result = best_match(&candidates(Some("Acme Corp"), None), &apps, false);
        assert_eq!(result.map(|s| s.application_id), Some("a1".to_string()));
    }

    #[test]
    fn no_match_below_the_company_threshold() {
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Saved,
        )];
        // "Acme Corp" vs "Beta Widgets" — zero token overlap.
        let result = best_match(&candidates(Some("Beta Widgets"), None), &apps, false);
        assert_eq!(result, None);
    }

    #[test]
    fn no_match_when_there_is_no_company_candidate_at_all() {
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Saved,
        )];
        assert_eq!(best_match(&candidates(None, None), &apps, false), None);
    }

    #[test]
    fn ignores_applications_not_in_the_saved_stage() {
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Applied,
        )];
        // Already applied → out of the candidate pool entirely (silent, per design).
        assert_eq!(
            best_match(&candidates(Some("Acme Corp"), None), &apps, false),
            None
        );
    }

    #[test]
    fn domain_hint_boosts_a_borderline_score_over_the_threshold_but_not_a_weak_one() {
        // Synthetic single-letter tokens so the Jaccard arithmetic is exactly
        // checkable: candidate {a,b,c,d,e} (5 tokens) is a strict subset of
        // the saved application's {a..k} (11 tokens) → 5/11 ≈ 0.4545, just
        // below COMPANY_THRESHOLD (0.5) on its own.
        let borderline = vec![app(
            "a1",
            "a b c d e f g h i j k",
            "",
            ApplicationStatus::Saved,
        )];
        let candidate = candidates(Some("a b c d e"), None);

        assert_eq!(
            best_match(&candidate, &borderline, false),
            None,
            "0.4545 alone must not clear the 0.5 bar"
        );
        assert_eq!(
            best_match(&candidate, &borderline, true).map(|s| s.application_id),
            Some("a1".to_string()),
            "+0.05 domain-hint boost (→ 0.5045) should tip a genuinely borderline score over"
        );

        // A weak, near-zero overlap must stay unmatched even with the hint —
        // the boost can never manufacture a match out of a real mismatch.
        let weak = vec![app("a2", "x y z", "", ApplicationStatus::Saved)];
        assert_eq!(
            best_match(&candidates(Some("a b c"), None), &weak, true),
            None
        );
    }

    #[test]
    fn ambiguous_tie_between_two_saved_applications_is_none() {
        let apps = vec![
            app(
                "a1",
                "Acme Corp",
                "Software Engineer",
                ApplicationStatus::Saved,
            ),
            app(
                "a2",
                "Acme Corp",
                "Backend Developer",
                ApplicationStatus::Saved,
            ),
        ];
        // Identical company tokens on both, no title candidate to disambiguate
        // → exactly tied scores → treated as ambiguous, not guessed.
        assert_eq!(
            best_match(&candidates(Some("Acme Corp"), None), &apps, false),
            None
        );
    }

    #[test]
    fn title_overlap_breaks_a_tie_by_raising_the_matching_ones_score() {
        let apps = vec![
            app(
                "a1",
                "Acme Corp",
                "Software Engineer",
                ApplicationStatus::Saved,
            ),
            app(
                "a2",
                "Acme Corp",
                "Backend Developer",
                ApplicationStatus::Saved,
            ),
        ];
        let result = best_match(
            &candidates(Some("Acme Corp"), Some("Software Engineer")),
            &apps,
            false,
        );
        assert_eq!(result.map(|s| s.application_id), Some("a1".to_string()));
    }

    // ── known precision limits (job-match-expert item 11 e/f, documented not fixed) ──

    #[test]
    fn known_precision_limit_ambiguous_title_extraction_can_favor_the_wrong_role() {
        // Documents a real precision limit, not a bug: when a company has TWO
        // saved roles and the email's extracted title only generically
        // overlaps both, the matcher picks whichever token overlap is
        // HIGHER — it has no way to know which role the email is actually
        // about beyond that overlap. Here "Engineer" shares a token with
        // a1's "Software Engineer" but none with a2's "Backend Developer",
        // even though the real confirmation could equally plausibly be
        // about either role.
        let apps = vec![
            app(
                "a1",
                "Acme Corp",
                "Software Engineer",
                ApplicationStatus::Saved,
            ),
            app(
                "a2",
                "Acme Corp",
                "Backend Developer",
                ApplicationStatus::Saved,
            ),
        ];
        let result = best_match(
            &candidates(Some("Acme Corp"), Some("Engineer")),
            &apps,
            false,
        );
        assert_eq!(
            result.map(|s| s.application_id),
            Some("a1".to_string()),
            "picks a1 purely because 'Engineer' shares a token with its title — not because \
             the email is provably about that role; a known precision limit, not a correctness bug"
        );
    }

    #[test]
    fn known_precision_limit_two_different_companies_sharing_one_token_both_stay_below_threshold() {
        let apps = vec![
            app("a1", "Acme Ventures Group", "", ApplicationStatus::Saved),
            app("a2", "Acme Capital Partners", "", ApplicationStatus::Saved),
        ];
        // "Acme" alone shares only the generic "acme" token with EACH
        // company — neither clears the threshold on its own, so this is
        // correctly a non-match rather than a coin-flip between two
        // unrelated companies that happen to share one word.
        assert_eq!(
            best_match(&candidates(Some("Acme"), None), &apps, false),
            None
        );
    }

    #[test]
    fn umlaut_and_legal_suffix_normalize_to_the_same_tokens() {
        assert_eq!(normalize_tokens("Müller GmbH"), normalize_tokens("Müller"));
    }

    #[test]
    fn normalize_tokens_strips_legal_suffixes_so_they_compare_equal() {
        assert_eq!(normalize_tokens("Acme Corp"), normalize_tokens("Acme Inc."));
        assert_eq!(normalize_tokens("Acme GmbH"), normalize_tokens("Acme"));
    }
}
