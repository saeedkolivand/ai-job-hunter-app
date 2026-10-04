use super::*;

// ── the invariant that replaces the old knock-out count ───────────────────

/// No input reaches `NotMet` through the shipped checks.
///
/// The inverse of the assertion this test replaces. The old one counted the
/// knock-outs a hand-picked corpus produced and asserted the count — which
/// could only ever confirm the code agreed with itself, because the corpus
/// held only pairings it already agreed with. This asserts an absolute zero
/// over a corpus deliberately stocked with the pairs that used to fail, and
/// anchors the non-vacuity from both ends: every cell yields exactly one
/// check, and a known number of them are `Met`.
#[test]
fn no_shipped_constraint_can_emit_a_knock_out() {
    let places = [
        None,
        Some(""),
        Some("Berlin, Germany"),
        Some("Austin, TX"),
        Some("Remote"),
        Some("München"),
        Some("Wien"),
        Some("Multiple locations"),
        Some("EMEA"),
        Some("東京"),
        // Carries the partial-overlap pairing (with the "San Francisco"
        // preference below) so the strict-Met rule is exercised by the
        // distribution, not only by the dedicated test.
        Some("San Diego, CA"),
    ];
    let prefs = [
        None,
        Some(""),
        Some("Berlin"),
        Some("DE"),
        Some("Munich"),
        Some("Vienna"),
        Some("Germany"),
        Some("San Francisco"),
    ];
    let (mut total, mut met, mut unknown, mut no_pref) = (0, 0, 0, 0);
    for p in places {
        for c in prefs {
            for remote in [false, true] {
                let checks = evaluate(&posting(p, remote), &candidate(c));
                assert_eq!(checks.len(), 1, "each cell yields exactly one check");
                for check in checks {
                    total += 1;
                    match check.status() {
                        ConstraintStatus::NotMet => {
                            panic!("no shipped constraint may accuse: {check:?}")
                        }
                        ConstraintStatus::Met => met += 1,
                        ConstraintStatus::Unknown => unknown += 1,
                        ConstraintStatus::NoPreference => no_pref += 1,
                    }
                }
            }
        }
    }
    // The whole distribution, hand-derived, so this cannot pass by the
    // corpus quietly collapsing to one answer. 11 places × 8 preferences
    // × 2 remote flags.
    assert_eq!(total, 176);
    // 2 blank preferences × 11 places × 2 flags — checked before anything
    // about the posting is read.
    assert_eq!(no_pref, 44);
    // 66 from board_remote=true with a stored preference (6 × 11), plus 9
    // at remote=false: "Berlin, Germany" for both Berlin and Germany,
    // "München" for Munich via the exonym table, and "Remote" for all 6
    // non-blank preferences via the marker list.
    //
    // NOT among them: "San Francisco" against "San Diego, CA". That pair
    // shares the `san` token and was `met` under the loose reading — it is
    // the cell that fails this assertion if the strict whole-token rule is
    // ever relaxed.
    assert_eq!(met, 66 + 9);
    // Everything left over — including every pair in the table above.
    assert_eq!(unknown, 57);
    assert_eq!(met + unknown + no_pref, total);
}

/// The constructor refuses to build a one-sided accusation. Driven through
/// `ConstraintCheck::new` — the only reachable way any check is ever built,
/// now that the type is sealed in its own module — so this cannot pass while
/// the production path bypasses the guard.
#[test]
fn an_unevidenced_accusation_is_downgraded_to_unknown_at_construction() {
    let some = || Some("Austin, TX".to_string());
    let one_sided = [
        (None, Some("Berlin".to_string())),         // posting silent
        (some(), None),                             // candidate silent
        (Some("  ".to_string()), Some("B".into())), // blank is not evidence
        (some(), Some("   ".to_string())),
    ];
    for (posting, candidate) in one_sided {
        let check = ConstraintCheck::new(
            PREFERRED_LOCATION,
            ConstraintStatus::NotMet,
            posting,
            candidate,
        );
        assert_eq!(
            check.status(),
            ConstraintStatus::Unknown,
            "a knock-out without evidence on both sides must not survive construction: {check:?}"
        );
    }
    // A fully-evidenced knock-out is left alone — the floor is two-sided
    // evidence, and judging whether that evidence is a CONFLICT is each
    // check's own job (which is why `location_check` never asks for one).
    let real = ConstraintCheck::new(
        PREFERRED_LOCATION,
        ConstraintStatus::NotMet,
        some(),
        Some("Berlin".to_string()),
    );
    assert_eq!(real.status(), ConstraintStatus::NotMet);
    // The guard only ever touches NotMet: a one-sided Met/Unknown/
    // NoPreference is legitimate and passes through unchanged.
    for status in [
        ConstraintStatus::Met,
        ConstraintStatus::Unknown,
        ConstraintStatus::NoPreference,
    ] {
        let check = ConstraintCheck::new(PREFERRED_LOCATION, status, None, None);
        assert_eq!(check.status(), status);
    }
}
