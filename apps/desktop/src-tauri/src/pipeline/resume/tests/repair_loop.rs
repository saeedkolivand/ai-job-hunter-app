use super::super::stages::sections;
use super::super::types::SectionKey;
use super::support::{
    live_deadline, repair_report, repair_revalidate, REPAIR_DRAFT, REPAIR_FIXED_SUMMARY,
};

/// **The happy path, whole.** One round, one section, the splice lands, the
/// re-validation clears the Critical, and the loop stops because there is
/// nothing left to fix — not because it ran out of rounds.
///
/// Mutation check: make the loop keep the candidate WITHOUT re-validating (drop
/// the `report = candidate_report` assignment) and the "no criticals remain"
/// assertion fails; drop the `after == 0` break and `rounds` becomes 2.
#[tokio::test]
async fn the_repair_loop_splices_revalidates_and_stops_when_clean() {
    let mut calls = 0u32;
    let (document, report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |key, document, _issues| {
            calls += 1;
            assert_eq!(
                key,
                SectionKey::Summary,
                "only the failing section is asked"
            );
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the summary exists");
            let spliced = sections::splice(&document, section, REPAIR_FIXED_SUMMARY);
            async move { Ok(super::super::stages::SectionOutcome::Replaced(spliced)) }
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("the loop only errors when re-validation cannot run");

    assert_eq!(calls, 1, "one failing section, one call");
    assert_eq!(stats.rounds, 1);
    assert_eq!(stats.calls, 1);
    assert!(!stats.reverted);
    assert!(!stats.timed_out && !stats.budgeted);
    assert!(
        !document.contains("47%"),
        "the corrected section must actually be in the document"
    );
    assert!(
        document.contains("Built the ledger service"),
        "the untouched sections survive the splice"
    );
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|i| i.severity == crate::validate::Severity::Critical)
            .count(),
        0,
        "the report the loop returns is the one it validated, not the one it started with"
    );
}

/// **A round that makes things worse is reverted, totally.** The candidate is a
/// clone, so the revert is the absence of a write rather than a rollback — and
/// the loop stops there rather than spending its second round.
///
/// Mutation check: assign `draft = candidate` before the `round_is_worse` check
/// and the "original document survives" assertion fails.
#[tokio::test]
async fn the_repair_loop_reverts_a_round_that_adds_criticals() {
    // A "correction" that invents MORE unsourced figures than it removed.
    // Three-digit-or-percent figures on purpose: `metrics_in` deliberately
    // ignores bare numbers under three digits with no `%`/`x` unit, so "30
    // teams" would count for nothing and the round would not be worse at all.
    let worse = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 61% across 340 teams in 125 markets.";
    let (document, report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |key, document, _issues| {
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the summary exists");
            let spliced = sections::splice(&document, section, worse);
            async move { Ok(super::super::stages::SectionOutcome::Replaced(spliced)) }
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("re-validation ran");

    assert!(stats.reverted, "a strictly-worse round must revert");
    assert_eq!(stats.rounds, 1, "…and stop, not spend the second round");
    assert_eq!(
        document, REPAIR_DRAFT,
        "the ORIGINAL document survives byte-for-byte — the round worked on a clone"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.severity == crate::validate::Severity::Critical),
        "the reverted round's report must not be kept either"
    );
}
