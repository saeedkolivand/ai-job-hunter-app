use super::*;

// ── location: the positive case ───────────────────────────────────────────

#[test]
fn location_is_met_when_the_posting_names_the_candidates_stored_place() {
    let checks = evaluate(
        &posting(Some("Berlin, Germany"), false),
        &candidate(Some("Berlin")),
    );
    let check = only(&checks);
    assert_eq!(check.status(), ConstraintStatus::Met);
    // Both sides' own words are carried, verbatim.
    assert_eq!(check.posting(), Some("Berlin, Germany"));
    assert_eq!(check.candidate(), Some("Berlin"));
}

#[test]
fn a_remote_posting_is_met_against_any_stored_place() {
    // By the board flag, even with a far-away location text …
    assert_eq!(
        status_of("Berlin", "Austin, TX", true),
        ConstraintStatus::Met
    );
    // … and by a marker in the location text alone.
    assert_eq!(
        status_of("Berlin", "Remote — Worldwide", false),
        ConstraintStatus::Met
    );
}

#[test]
fn location_is_met_across_a_curated_exonym_pair() {
    // Proof the shared matcher is what decides: "Munich" ↔ "München" is
    // bridged only by `location_filter`'s curated table, which a forked
    // matcher here would not have.
    assert_eq!(
        status_of("Munich", "München, Bayern", false),
        ConstraintStatus::Met
    );
}

// ── location: everything else is Unknown, never a knock-out ───────────────

/// The correction this module was rewritten for.
///
/// Every pair below is a job the candidate could plausibly take, and every
/// one of them made the FIRST cut of this module publish `notMet` — the
/// strongest negative signal the payload has. A failed substring search is
/// absence of evidence, not evidence of conflict, so all of them are
/// `Unknown` now.
#[test]
fn a_failed_place_name_match_is_unknown_not_a_knock_out() {
    // (stored preference, posting location, why the substring search fails)
    let takeable: &[(&str, &str, &str)] = &[
        ("Germany", "Berlin", "granularity: country vs city"),
        ("Berlin", "Germany", "granularity: city vs country"),
        ("Vienna", "Wien", "exonym outside the curated table"),
        ("NYC", "New York, NY", "abbreviation"),
        ("San Francisco", "Bay Area", "metro synonym"),
        ("Tokyo", "東京", "non-Latin script"),
        (
            "Berlin",
            "Telecommute",
            "remote synonym off the marker list",
        ),
        ("Berlin", "Virtual", "remote synonym off the marker list"),
        ("Berlin", "Multiple locations", "not a place name"),
        ("Berlin", "EMEA", "region, not a place name"),
        (
            "Berlin",
            "HQ: Austin, TX",
            "unknowable from a settings field",
        ),
    ];
    for (pref, loc, why) in takeable {
        let got = status_of(pref, loc, false);
        assert_eq!(
            got,
            ConstraintStatus::Unknown,
            "{pref:?} vs {loc:?} ({why}) must be unknown, never a knock-out"
        );
    }
    // The three the review named explicitly, asserted individually so a
    // corpus edit cannot quietly drop them.
    assert_eq!(
        status_of("Germany", "Berlin", false),
        ConstraintStatus::Unknown
    );
    assert_eq!(
        status_of("Vienna", "Wien", false),
        ConstraintStatus::Unknown
    );
    assert_eq!(
        status_of("Berlin", "Multiple locations", false),
        ConstraintStatus::Unknown
    );
}

/// A partial place-name overlap is not agreement.
///
/// The scrape filter deliberately keeps a "San Diego" row for a "San
/// Francisco" search — discarding a job is the expensive mistake there. This
/// pass must not turn that same conservatism into a claim, because here the
/// expensive mistake is telling the user something false. Same matcher,
/// opposite risk, so the epistemics differ.
#[test]
fn a_partial_place_name_overlap_is_unknown_not_met() {
    // (preference, posting location, what is only partially shared)
    let partial: &[(&str, &str, &str)] = &[
        ("San Francisco", "San Diego, CA", "the `san` token only"),
        ("San Francisco", "Santa Monica, CA", "`san` inside `santa`"),
        (
            "Berlin, Germany",
            "Berlin",
            "a requested token with no home",
        ),
        ("New York", "Newark, NJ", "`new` inside `newark`"),
    ];
    for (pref, loc, why) in partial {
        assert_eq!(
            status_of(pref, loc, false),
            ConstraintStatus::Unknown,
            "{pref:?} vs {loc:?} ({why}) must not read as a match"
        );
    }
    // The control: the SAME preference against a posting that really does
    // name it reads met, so this is a strictness rule and not a blanket
    // refusal to ever match a two-token place.
    assert_eq!(
        status_of("San Francisco", "San Francisco, CA", false),
        ConstraintStatus::Met
    );
}

#[test]
fn location_is_unknown_when_the_posting_states_no_location() {
    for absent in [None, Some(""), Some("   ")] {
        let checks = evaluate(&posting(absent, false), &candidate(Some("Berlin")));
        let check = only(&checks);
        assert_eq!(
            check.status(),
            ConstraintStatus::Unknown,
            "posting location {absent:?} must be unknown, never a pass or a fail"
        );
        assert_eq!(check.posting(), None);
        assert_eq!(check.candidate(), Some("Berlin"));
    }
}

#[test]
fn location_is_no_preference_when_the_candidate_stored_nothing() {
    for empty in [None, Some(""), Some("  ")] {
        let checks = evaluate(&posting(Some("Austin, TX"), false), &candidate(empty));
        let check = only(&checks);
        // Distinct from Unknown: the user can fix this one.
        assert_eq!(check.status(), ConstraintStatus::NoPreference);
        assert_ne!(check.status(), ConstraintStatus::Unknown);
        assert_eq!(check.candidate(), None);
        // The posting's evidence is still reported.
        assert_eq!(check.posting(), Some("Austin, TX"));
    }
}

/// The wire id names what is actually compared.
///
/// `preferredLocation`, not `location`: this check reads the user's stored
/// Preferred Location SETTING, and the id is what a renderer keys its i18n
/// off. Renaming it to `location` would license "you can't work there" /
/// "this posting matches where you're looking" — sentences the data does not
/// support — so the id is pinned rather than left to drift.
#[test]
fn the_check_is_named_for_the_setting_it_reads() {
    let checks = evaluate(
        &posting(Some("Berlin, Germany"), false),
        &candidate(Some("Berlin")),
    );
    assert_eq!(checks[0].id(), "preferredLocation");
    assert_ne!(
        checks[0].id(),
        "location",
        "`location` would read as a claim about where the user can work"
    );
}

/// The shipped UI location defaults must produce `met`.
///
/// Every one of them ends in a two-letter qualifier, and every one reads as
/// a match ONLY because `MIN_TOKEN_LEN` (3) drops that qualifier before
/// comparison. That constant's own doc justifies 3 as scrape-filter noise
/// control and says nothing about a published verdict depending on it, so
/// lowering it to 2 for a scrape-side reason would flip all three of these
/// to `unknown` with nothing else failing. This is the test that fails.
#[test]
fn the_shipped_ui_location_defaults_read_as_a_match() {
    // (JobLocationPreferences' COMMON_LOCATIONS entry, a realistic posting)
    let defaults: &[(&str, &str)] = &[
        ("San Francisco, CA", "San Francisco, California"),
        ("New York, NY", "New York, United States"),
        ("London, UK", "London, United Kingdom"),
        // No two-letter qualifier, included so the case is covered either way.
        ("Berlin, Germany", "Berlin, Germany"),
    ];
    for (pref, posting_location) in defaults {
        assert_eq!(
            status_of(pref, posting_location, false),
            ConstraintStatus::Met,
            "the shipped default {pref:?} must still match {posting_location:?}"
        );
    }
}

/// The verdict and the evidence are derived from the SAME bytes.
///
/// Re-running the evaluation on exactly what the check REPORTS must
/// reproduce exactly what it decided. When the two came from different bytes
/// — an unclamped location compared, a clamped one reported — a location
/// longer than the cap could match on a token that then fell outside the
/// evidence, leaving `met` beside a string not containing the place that
/// produced it. The board flag is passed through since it is not evidence
/// text.
#[test]
fn a_verdict_is_reproducible_from_the_evidence_it_reports() {
    let long_tail = format!("{} Berlin", "x".repeat(300));
    let long_head = format!("Berlin {}", "x".repeat(300));
    let places = [
        None,
        Some("Berlin, Germany"),
        Some("Austin, TX"),
        Some("Remote"),
        Some(long_tail.as_str()),
        Some(long_head.as_str()),
    ];
    // A preference LONGER than the cap belongs here too: the candidate side
    // is clamped for the payload exactly like the posting side, so comparing
    // the unclamped preference would break the same property in the mirror
    // direction. This one puts the meaningful token past the cut.
    let long_pref_tail = format!("{} Berlin", "x".repeat(300));
    let prefs = [
        None,
        Some("Berlin"),
        Some("San Francisco"),
        Some(long_pref_tail.as_str()),
    ];
    let mut checked = 0;
    for p in places {
        for c in prefs {
            for remote in [false, true] {
                let facts = posting(p, remote);
                let cand = candidate(c);
                let check = &evaluate(&facts, &cand)[0];
                // Rebuild both sides from what was REPORTED and re-decide.
                let replayed = &evaluate(
                    &PostingFacts {
                        location: check.posting().map(str::to_string),
                        board_remote: facts.board_remote,
                    },
                    &CandidateFacts {
                        location: check.candidate().map(str::to_string),
                    },
                )[0];
                assert_eq!(
                    check.status(),
                    replayed.status(),
                    "status must be reproducible from the reported evidence: {check:?}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 48); // 6 places × 4 preferences × 2 flags
                             // The case that used to disagree, pinned concretely: the matching token
                             // sits past the byte cap, so it is NOT in the evidence and must NOT
                             // produce a match.
    let past_the_cap = &evaluate(
        &posting(Some(&long_tail), false),
        &candidate(Some("Berlin")),
    )[0];
    assert_eq!(past_the_cap.status(), ConstraintStatus::Unknown);
    assert!(!past_the_cap.posting().unwrap().contains("Berlin"));
    // …while the same token INSIDE the cap still matches.
    let inside_the_cap = &evaluate(
        &posting(Some(&long_head), false),
        &candidate(Some("Berlin")),
    )[0];
    assert_eq!(inside_the_cap.status(), ConstraintStatus::Met);
    assert!(inside_the_cap.posting().unwrap().contains("Berlin"));
    // The mirror: an over-long PREFERENCE is clamped before comparison too,
    // so a token of it past the cut cannot produce a match it does not
    // report. Without this the candidate-side half of the fix is untested.
    let long_pref = &evaluate(
        &posting(Some("Berlin, Germany"), false),
        &candidate(Some(&long_pref_tail)),
    )[0];
    assert_eq!(long_pref.status(), ConstraintStatus::Unknown);
    assert!(!long_pref.candidate().unwrap().contains("Berlin"));
}
