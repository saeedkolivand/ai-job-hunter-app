use super::support::*;
use crate::insights::{report, Dimension, Outcome, OutcomeCounts, OutcomeGroup, OutcomeRates};

fn group<'a>(groups: &'a [OutcomeGroup], dimension: Dimension, key: &str) -> &'a OutcomeGroup {
    groups
        .iter()
        .find(|g| g.dimension == dimension && g.key == key)
        .unwrap_or_else(|| panic!("no {dimension:?}/{key} group in {groups:?}"))
}

fn board(inputs: &[crate::insights::AppInput], key: &str) -> OutcomeGroup {
    group(&report(inputs, NOW).groups, Dimension::Board, key).clone()
}

#[test]
fn four_decided_applications_report_counts_but_no_rate() {
    let mut inputs = many(2, "indeed", Outcome::PositiveReply, NOW);
    inputs.extend(many(2, "indeed", Outcome::NoReply, NOW));
    let g = board(&inputs, "indeed");
    assert_eq!(
        g.counts,
        OutcomeCounts {
            positive_reply: 2,
            rejected: 0,
            no_reply: 2,
            pending: 0
        }
    );
    assert_eq!(g.rates, None);
}

#[test]
fn five_decided_applications_report_rates_over_the_decided_set() {
    let mut inputs = many(2, "indeed", Outcome::PositiveReply, NOW);
    inputs.extend(many(1, "indeed", Outcome::Rejected, NOW));
    inputs.extend(many(2, "indeed", Outcome::NoReply, NOW));
    let g = board(&inputs, "indeed");
    assert_eq!(
        g.rates,
        Some(OutcomeRates {
            positive_reply: 0.4,
            rejected: 0.2,
            no_reply: 0.4
        })
    );
}

#[test]
fn pending_applications_are_counted_but_never_enter_the_rate() {
    // Four decided plus three pending: below the minimum, so still no rate.
    let mut inputs = many(4, "indeed", Outcome::Rejected, NOW);
    inputs.extend(many(3, "indeed", Outcome::Pending, NOW));
    let g = board(&inputs, "indeed");
    assert_eq!(g.counts.pending, 3);
    assert_eq!(g.rates, None);

    // A fifth decided one unlocks the rate, computed over 5 and not over 8.
    inputs.extend(many(1, "indeed", Outcome::PositiveReply, NOW));
    let g = board(&inputs, "indeed");
    assert_eq!(
        g.rates,
        Some(OutcomeRates {
            positive_reply: 0.2,
            rejected: 0.8,
            no_reply: 0.0
        })
    );
}

#[test]
fn excluded_applications_appear_in_no_group() {
    let inputs = vec![input(NOW, "indeed", None, None)];
    assert!(report(&inputs, NOW).groups.is_empty());
}

#[test]
fn board_groups_split_by_board_and_blank_boards_are_unknown() {
    let inputs = vec![
        input(NOW, "indeed", Some(Outcome::Rejected), None),
        input(NOW, "linkedin", Some(Outcome::Rejected), None),
        input(NOW, "  ", Some(Outcome::Rejected), None),
    ];
    let groups = report(&inputs, NOW).groups;
    for key in ["indeed", "linkedin", "unknown"] {
        assert_eq!(
            group(&groups, Dimension::Board, key).counts.rejected,
            1,
            "{key}"
        );
    }
}

#[test]
fn tailored_and_cover_letter_groups_follow_the_generation() {
    let inputs = vec![
        input(
            NOW,
            "b",
            Some(Outcome::PositiveReply),
            Some(generation("en", None, "A letter.")),
        ),
        input(
            NOW,
            "b",
            Some(Outcome::Rejected),
            Some(generation("en", None, "   ")),
        ),
        input(NOW, "b", Some(Outcome::NoReply), None),
    ];
    let groups = report(&inputs, NOW).groups;
    assert_eq!(
        group(&groups, Dimension::Tailored, "tailored")
            .counts
            .positive_reply,
        1
    );
    assert_eq!(
        group(&groups, Dimension::Tailored, "tailored")
            .counts
            .rejected,
        1
    );
    assert_eq!(
        group(&groups, Dimension::Tailored, "untailored")
            .counts
            .no_reply,
        1
    );
    assert_eq!(
        group(&groups, Dimension::CoverLetter, "included")
            .counts
            .positive_reply,
        1
    );
    // A blank letter and no generation at all are both "none".
    assert_eq!(
        group(&groups, Dimension::CoverLetter, "none")
            .counts
            .rejected,
        1
    );
    assert_eq!(
        group(&groups, Dimension::CoverLetter, "none")
            .counts
            .no_reply,
        1
    );
}

#[test]
fn target_language_group_uses_the_generation_language_or_unknown() {
    let inputs = vec![
        input(
            NOW,
            "b",
            Some(Outcome::Rejected),
            Some(generation("de", None, "")),
        ),
        input(NOW, "b", Some(Outcome::Rejected), None),
    ];
    let groups = report(&inputs, NOW).groups;
    assert_eq!(
        group(&groups, Dimension::TargetLanguage, "de")
            .counts
            .rejected,
        1
    );
    assert_eq!(
        group(&groups, Dimension::TargetLanguage, "unknown")
            .counts
            .rejected,
        1
    );
}

#[test]
fn template_group_buckets_rows_without_a_template_as_unknown() {
    let inputs = vec![
        input(
            NOW,
            "b",
            Some(Outcome::PositiveReply),
            Some(generation("en", Some("swiss-minimal"), "")),
        ),
        input(
            NOW,
            "b",
            Some(Outcome::Rejected),
            Some(generation("en", None, "")),
        ),
        input(NOW, "b", Some(Outcome::Rejected), None),
    ];
    let groups = report(&inputs, NOW).groups;
    assert_eq!(
        group(&groups, Dimension::Template, "swiss-minimal")
            .counts
            .positive_reply,
        1
    );
    assert_eq!(
        group(&groups, Dimension::Template, "unknown")
            .counts
            .rejected,
        2
    );
}

#[test]
fn month_group_is_the_utc_month_and_flips_at_midnight() {
    // 2026-04-01T00:00:00.000Z is 1_775_001_600_000 ms.
    let inputs = vec![
        input(1_775_001_599_999, "b", Some(Outcome::Rejected), None),
        input(1_775_001_600_000, "b", Some(Outcome::Rejected), None),
    ];
    let groups = report(&inputs, NOW).groups;
    assert_eq!(
        group(&groups, Dimension::Month, "2026-03").counts.rejected,
        1
    );
    assert_eq!(
        group(&groups, Dimension::Month, "2026-04").counts.rejected,
        1
    );
}

#[test]
fn groups_are_ordered_by_dimension_then_key_and_the_report_carries_its_time() {
    let inputs = vec![
        input(1_775_001_600_000, "zeta", Some(Outcome::Rejected), None),
        input(1_775_001_599_999, "alpha", Some(Outcome::Rejected), None),
    ];
    let r = report(&inputs, NOW);
    let order: Vec<(Dimension, &str)> = r
        .groups
        .iter()
        .map(|g| (g.dimension, g.key.as_str()))
        .collect();
    assert_eq!(
        order,
        vec![
            (Dimension::Board, "alpha"),
            (Dimension::Board, "zeta"),
            (Dimension::Tailored, "untailored"),
            (Dimension::CoverLetter, "none"),
            (Dimension::TargetLanguage, "unknown"),
            (Dimension::Month, "2026-03"),
            (Dimension::Month, "2026-04"),
            (Dimension::Template, "unknown"),
        ]
    );
    assert_eq!(r.generated_at, NOW);
}

#[test]
fn an_oversized_group_key_is_capped_on_a_char_boundary() {
    // 3-byte chars: 64 bytes falls mid-character, so the cap must back off to 63.
    let long = "€".repeat(100);
    let inputs = vec![input(NOW, &long, Some(Outcome::Rejected), None)];
    let groups = report(&inputs, NOW).groups;
    let key = &groups[0].key;
    assert_eq!(groups[0].dimension, Dimension::Board);
    assert_eq!(key.len(), 63);
    assert_eq!(*key, "€".repeat(21));

    let ascii = "a".repeat(500);
    let inputs = vec![input(NOW, &ascii, Some(Outcome::Rejected), None)];
    assert_eq!(report(&inputs, NOW).groups[0].key.len(), 64);
}
