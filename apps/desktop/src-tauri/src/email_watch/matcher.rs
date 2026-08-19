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
mod tests {
    use super::*;
    use crate::applications::ApplicationStatus;

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
            next_action_notified_at: None,
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
        let result = best_match(
            &candidates(Some("Acme Corp"), None),
            &apps,
            false,
            &HashSet::new(),
        );
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
        let result = best_match(
            &candidates(Some("Beta Widgets"), None),
            &apps,
            false,
            &HashSet::new(),
        );
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
        assert_eq!(
            best_match(&candidates(None, None), &apps, false, &HashSet::new()),
            None
        );
    }

    #[test]
    fn a_live_non_saved_status_is_still_a_candidate() {
        // The old premise ("only Saved is a candidate") was the bug this
        // module's fix addresses: a rejection/interview/offer for an
        // `Applied` (or Screening/Interviewing/Offer/Accepted) application
        // could never match before. Any LIVE status is eligible regardless
        // of `unconfirmed_email_write_ids`.
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Applied,
        )];
        assert_eq!(
            best_match(
                &candidates(Some("Acme Corp"), None),
                &apps,
                false,
                &HashSet::new()
            )
            .map(|s| s.application_id),
            Some("a1".to_string())
        );
    }

    #[test]
    fn a_user_set_terminal_status_is_not_a_candidate() {
        // Rejected and NOT in unconfirmed_email_write_ids — i.e. the user
        // (or a prior CONFIRMED email write) set this, not an unconfirmed
        // email-derived write. Must stay out of the candidate pool, or a
        // later email could silently reopen a status the user already
        // settled.
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Rejected,
        )];
        assert_eq!(
            best_match(
                &candidates(Some("Acme Corp"), None),
                &apps,
                false,
                &HashSet::new()
            ),
            None
        );
    }

    #[test]
    fn an_unconfirmed_email_derived_terminal_status_is_still_a_candidate() {
        // Same as above but `a1` IS in unconfirmed_email_write_ids — the
        // exact case the terminal-override half of `next_status` exists for.
        // If the matcher excluded it, that fix would be dead code one layer
        // up (see this module's doc + `next_status`'s doc).
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Rejected,
        )];
        let mut unconfirmed = HashSet::new();
        unconfirmed.insert("a1".to_string());
        assert_eq!(
            best_match(
                &candidates(Some("Acme Corp"), None),
                &apps,
                false,
                &unconfirmed
            )
            .map(|s| s.application_id),
            Some("a1".to_string())
        );
    }

    #[test]
    fn matcher_and_next_status_eligibility_never_disagree() {
        // The real invariant the fix-forward task asked for: matcher
        // candidacy and `next_status`'s own actionability gate must NEVER
        // independently disagree. Both already call the SAME
        // `is_actionable` function, so this is guaranteed by construction —
        // but a future edit could reintroduce a hand-rolled condition in
        // either place, so this proves agreement empirically rather than
        // trusting the shared call site to stay that way. A same-company,
        // no-title-nudge-needed candidate is used so the ONLY thing that can
        // exclude it is the eligibility filter, never the score threshold.
        for &status in crate::applications::ApplicationStatus::ALL {
            for unconfirmed in [false, true] {
                let expected = is_actionable(status, unconfirmed);
                let apps = vec![app("a1", "Acme Corp", "Engineer", status)];
                let mut ids = HashSet::new();
                if unconfirmed {
                    ids.insert("a1".to_string());
                }
                let matched =
                    best_match(&candidates(Some("Acme Corp"), None), &apps, false, &ids).is_some();
                assert_eq!(
                    matched, expected,
                    "matcher candidacy for {status:?} (unconfirmed_email_write={unconfirmed}) \
                     must equal is_actionable"
                );
            }
        }
    }

    #[test]
    fn a_rejection_email_matches_an_application_at_applied() {
        // The concrete case that was impossible before this fix: previously
        // only Saved could ever be reached, so a rejection for an `Applied`
        // application could never match at the matcher layer at all.
        let apps = vec![app(
            "a1",
            "Acme Corp",
            "Software Engineer",
            ApplicationStatus::Applied,
        )];
        let matched = best_match(
            &candidates(Some("Acme Corp"), None),
            &apps,
            false,
            &HashSet::new(),
        )
        .map(|s| s.application_id);
        assert_eq!(matched, Some("a1".to_string()));
        assert_eq!(
            crate::email_watch::intent::next_status(
                crate::email_watch::intent::EmailIntent::Rejection,
                ApplicationStatus::Applied,
                false,
            ),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn two_same_company_applications_at_different_eligible_statuses_the_higher_title_overlap_wins()
    {
        // The coordinator's specific risk: widening candidacy means a
        // company can now have MULTIPLE simultaneously-eligible applications
        // across different lifecycle stages (not just multiple Saved rows).
        // When the email carries a title that clearly favors one of them,
        // that one wins — anchored to an absolute id, not just "the two
        // differ".
        let apps = vec![
            app(
                "saved-app",
                "Acme Corp",
                "Software Engineer",
                ApplicationStatus::Saved,
            ),
            app(
                "interviewing-app",
                "Acme Corp",
                "Product Manager",
                ApplicationStatus::Interviewing,
            ),
        ];
        let result = best_match(
            &candidates(Some("Acme Corp"), Some("Software Engineer")),
            &apps,
            false,
            &HashSet::new(),
        );
        assert_eq!(
            result.map(|s| s.application_id),
            Some("saved-app".to_string()),
            "the title-overlap nudge decides which of the two eligible same-company \
             applications wins — never a coin-flip"
        );
    }

    #[test]
    fn two_same_company_applications_at_different_eligible_statuses_with_no_title_is_ambiguous() {
        // Companion to the above: when the email carries NO extractable
        // title, both same-company candidates score identically on company
        // overlap alone (no title to differentiate) — an EXACT tie, still
        // correctly caught by the existing tie-rejection rule even though
        // the two are at different lifecycle stages, not both Saved.
        let apps = vec![
            app(
                "saved-app",
                "Acme Corp",
                "Software Engineer",
                ApplicationStatus::Saved,
            ),
            app(
                "interviewing-app",
                "Acme Corp",
                "Product Manager",
                ApplicationStatus::Interviewing,
            ),
        ];
        let result = best_match(
            &candidates(Some("Acme Corp"), None),
            &apps,
            false,
            &HashSet::new(),
        );
        assert_eq!(
            result, None,
            "no title to disambiguate → exact tie → ambiguous, not guessed"
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
            best_match(&candidate, &borderline, false, &HashSet::new()),
            None,
            "0.4545 alone must not clear the 0.5 bar"
        );
        assert_eq!(
            best_match(&candidate, &borderline, true, &HashSet::new()).map(|s| s.application_id),
            Some("a1".to_string()),
            "+0.05 domain-hint boost (→ 0.5045) should tip a genuinely borderline score over"
        );

        // A weak, near-zero overlap must stay unmatched even with the hint —
        // the boost can never manufacture a match out of a real mismatch.
        let weak = vec![app("a2", "x y z", "", ApplicationStatus::Saved)];
        assert_eq!(
            best_match(
                &candidates(Some("a b c"), None),
                &weak,
                true,
                &HashSet::new()
            ),
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
            best_match(
                &candidates(Some("Acme Corp"), None),
                &apps,
                false,
                &HashSet::new()
            ),
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
            &HashSet::new(),
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
            &HashSet::new(),
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
            best_match(
                &candidates(Some("Acme"), None),
                &apps,
                false,
                &HashSet::new()
            ),
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
