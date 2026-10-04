use super::*;
use crate::applications::ApplicationStatus;
use crate::email_watch::tests::support::application as app;

fn candidates(company: Option<&str>, title: Option<&str>) -> Candidates {
    Candidates {
        company: company.map(str::to_string),
        title: title.map(str::to_string),
    }
}

fn saved(id: &str, company: &str, title: &str) -> Application {
    app(id, company, title, ApplicationStatus::Saved)
}

/// `best_match` for `company`/`title` with no unconfirmed-email-write ids — the common case.
fn best(
    company: Option<&str>,
    title: Option<&str>,
    apps: &[Application],
    domain_hint: bool,
) -> Option<Scored> {
    best_match(
        &candidates(company, title),
        apps,
        domain_hint,
        &HashSet::new(),
    )
}

#[test]
fn matches_a_clear_company_overlap() {
    let apps = vec![saved("a1", "Acme Corp", "Software Engineer")];
    let result = best(Some("Acme Corp"), None, &apps, false);
    assert_eq!(result.map(|s| s.application_id), Some("a1".to_string()));
}

#[test]
fn no_match_below_the_company_threshold() {
    let apps = vec![saved("a1", "Acme Corp", "Software Engineer")];
    // "Acme Corp" vs "Beta Widgets" — zero token overlap.
    let result = best(Some("Beta Widgets"), None, &apps, false);
    assert_eq!(result, None);
}

#[test]
fn no_match_when_there_is_no_company_candidate_at_all() {
    let apps = vec![saved("a1", "Acme Corp", "Software Engineer")];
    assert_eq!(best(None, None, &apps, false), None);
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
        best(Some("Acme Corp"), None, &apps, false).map(|s| s.application_id),
        Some("a1".to_string())
    );
}

/// MAJOR fix: `Ghosted` used to be grouped with the hard-terminal
/// statuses in `status_ladder::is_live` (see that fn's own doc), so a
/// ghosted application was never even a match candidate — the email
/// was dropped before `next_status` ever saw it, regardless of what it
/// said. `ApplicationStatus::is_terminal` deliberately excludes
/// `Ghosted` ("a ghosted pursuit can still revive"); the matcher must
/// agree, unconditionally (no `unconfirmed_email_write` needed) — an
/// employer resurfacing after ghosting is exactly the case the domain
/// type exists for.
#[test]
fn a_ghosted_application_is_still_a_candidate() {
    let apps = vec![app(
        "a1",
        "Acme Corp",
        "Software Engineer",
        ApplicationStatus::Ghosted,
    )];
    assert_eq!(
        best(Some("Acme Corp"), None, &apps, false).map(|s| s.application_id),
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
    assert_eq!(best(Some("Acme Corp"), None, &apps, false), None);
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
    let matched = best(Some("Acme Corp"), None, &apps, false).map(|s| s.application_id);
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
fn two_same_company_applications_at_different_eligible_statuses_the_higher_title_overlap_wins() {
    // The coordinator's specific risk: widening candidacy means a
    // company can now have MULTIPLE simultaneously-eligible applications
    // across different lifecycle stages (not just multiple Saved rows).
    // When the email carries a title that clearly favors one of them,
    // that one wins — anchored to an absolute id, not just "the two
    // differ".
    let apps = vec![
        saved("saved-app", "Acme Corp", "Software Engineer"),
        app(
            "interviewing-app",
            "Acme Corp",
            "Product Manager",
            ApplicationStatus::Interviewing,
        ),
    ];
    let result = best(Some("Acme Corp"), Some("Software Engineer"), &apps, false);
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
        saved("saved-app", "Acme Corp", "Software Engineer"),
        app(
            "interviewing-app",
            "Acme Corp",
            "Product Manager",
            ApplicationStatus::Interviewing,
        ),
    ];
    let result = best(Some("Acme Corp"), None, &apps, false);
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
    let borderline = vec![saved("a1", "a b c d e f g h i j k", "")];
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
    let weak = vec![saved("a2", "x y z", "")];
    assert_eq!(best(Some("a b c"), None, &weak, true), None);
}

#[test]
fn ambiguous_tie_between_two_saved_applications_is_none() {
    let apps = vec![
        saved("a1", "Acme Corp", "Software Engineer"),
        saved("a2", "Acme Corp", "Backend Developer"),
    ];
    // Identical company tokens on both, no title candidate to disambiguate
    // → exactly tied scores → treated as ambiguous, not guessed.
    assert_eq!(best(Some("Acme Corp"), None, &apps, false), None);
}

#[test]
fn title_overlap_breaks_a_tie_by_raising_the_matching_ones_score() {
    let apps = vec![
        saved("a1", "Acme Corp", "Software Engineer"),
        saved("a2", "Acme Corp", "Backend Developer"),
    ];
    let result = best(Some("Acme Corp"), Some("Software Engineer"), &apps, false);
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
        saved("a1", "Acme Corp", "Software Engineer"),
        saved("a2", "Acme Corp", "Backend Developer"),
    ];
    let result = best(Some("Acme Corp"), Some("Engineer"), &apps, false);
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
        saved("a1", "Acme Ventures Group", ""),
        saved("a2", "Acme Capital Partners", ""),
    ];
    // "Acme" alone shares only the generic "acme" token with EACH
    // company — neither clears the threshold on its own, so this is
    // correctly a non-match rather than a coin-flip between two
    // unrelated companies that happen to share one word.
    assert_eq!(best(Some("Acme"), None, &apps, false), None);
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
