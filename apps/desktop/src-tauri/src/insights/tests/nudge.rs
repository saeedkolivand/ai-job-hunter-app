use super::support::*;
use crate::insights::{report, AppInput, NudgeTrigger, Outcome, VolumeNudge};

/// Content words no stop list or synonym table touches, so a letter's token set is exactly its words.
const WORDS: [&str; 12] = [
    "zorblat",
    "quimby",
    "frelnix",
    "wudgetron",
    "plimsoll",
    "ostrander",
    "vexilla",
    "turnbuckle",
    "yarrowick",
    "kestrelon",
    "marlowe",
    "ptolemaic",
];

fn letter(words: &[&str]) -> String {
    words.join(" ")
}

fn nudge(inputs: &[AppInput]) -> Option<VolumeNudge> {
    report(inputs, NOW).nudge
}

fn tailored(at: u64, text: &str) -> AppInput {
    input(
        at,
        "b",
        Some(Outcome::Pending),
        Some(generation("en", None, text)),
    )
}

fn untailored(at: u64) -> AppInput {
    input(at, "b", Some(Outcome::Pending), None)
}

/// Letters made of disjoint vocabularies: no two are near-copies.
fn distinct_letter(i: usize) -> String {
    letter(&[
        WORDS[i],
        WORDS[(i + 1) % 12],
        &format!("{}x", WORDS[(i + 2) % 12]),
    ])
}

#[test]
fn nine_applications_never_fire_however_untailored() {
    let inputs: Vec<_> = (0..9).map(|_| untailored(NOW)).collect();
    assert_eq!(nudge(&inputs), None);
}

#[test]
fn ten_untailored_applications_fire_on_the_untailored_condition() {
    let inputs: Vec<_> = (0..10).map(|_| untailored(NOW - 1000)).collect();
    assert_eq!(
        nudge(&inputs),
        Some(VolumeNudge {
            window_hours: 24,
            applications: 10,
            untailored: 10,
            letters: 0,
            near_copy_letters: 0,
            triggers: vec![NudgeTrigger::Untailored],
        })
    );
}

#[test]
fn exactly_half_untailored_does_not_fire_but_six_of_ten_does() {
    let mut inputs: Vec<_> = (0..5).map(|_| untailored(NOW)).collect();
    inputs.extend((0..5).map(|i| tailored(NOW, &distinct_letter(i))));
    assert_eq!(
        nudge(&inputs),
        None,
        "5 of 10 is not strictly more than half"
    );

    inputs[5] = untailored(NOW);
    let fired = nudge(&inputs).expect("6 of 10 untailored fires");
    assert_eq!(fired.untailored, 6);
    assert_eq!(fired.triggers, vec![NudgeTrigger::Untailored]);
}

#[test]
fn the_window_includes_exactly_24_hours_ago_and_excludes_one_ms_older() {
    let mut inputs: Vec<_> = (0..9).map(|_| untailored(NOW)).collect();
    inputs.push(untailored(NOW - 86_400_000));
    assert_eq!(nudge(&inputs).map(|n| n.applications), Some(10));

    inputs[9] = untailored(NOW - 86_400_001);
    assert_eq!(nudge(&inputs), None, "only 9 inside the window");
}

#[test]
fn applications_stamped_in_the_future_are_outside_the_window() {
    let mut inputs: Vec<_> = (0..9).map(|_| untailored(NOW)).collect();
    inputs.push(untailored(NOW + 1));
    assert_eq!(nudge(&inputs), None);
}

#[test]
fn withdrawn_applications_still_count_toward_the_window() {
    let mut inputs: Vec<_> = (0..9).map(|_| untailored(NOW)).collect();
    inputs.push(input(NOW, "b", None, None));
    assert_eq!(nudge(&inputs).map(|n| n.applications), Some(10));
}

#[test]
fn six_identical_letters_of_ten_fire_on_near_copies_alone() {
    let copy = letter(&WORDS[..8]);
    let mut inputs: Vec<_> = (0..6).map(|_| tailored(NOW, &copy)).collect();
    inputs.extend((0..4).map(|i| tailored(NOW, &distinct_letter(i))));
    assert_eq!(
        nudge(&inputs),
        Some(VolumeNudge {
            window_hours: 24,
            applications: 10,
            untailored: 0,
            letters: 10,
            near_copy_letters: 6,
            triggers: vec![NudgeTrigger::NearCopyLetters],
        })
    );
}

#[test]
fn exactly_half_the_letters_being_copies_does_not_fire() {
    let copy = letter(&WORDS[..8]);
    let mut inputs: Vec<_> = (0..5).map(|_| tailored(NOW, &copy)).collect();
    inputs.extend((0..5).map(|i| tailored(NOW, &distinct_letter(i))));
    assert_eq!(nudge(&inputs), None);
}

#[test]
fn near_copy_is_at_jaccard_point_eight_and_not_below() {
    // A 4-word base; one extra word on the other letter is 4/5 = 0.8 (a near-copy),
    // two extras are 4/6 (not). Eight untailored fillers make the window reach 10.
    let base = letter(&WORDS[..4]);
    let one_extra = letter(&WORDS[..5]);
    let two_extra = letter(&WORDS[..6]);
    let near_copies_of = |other: &str| {
        let mut inputs = vec![tailored(NOW, &base), tailored(NOW, other)];
        inputs.extend((0..8).map(|_| untailored(NOW)));
        nudge(&inputs).map(|n| n.near_copy_letters)
    };
    assert_eq!(near_copies_of(&one_extra), Some(2));
    assert_eq!(near_copies_of(&two_extra), Some(0));
}

#[test]
fn both_conditions_can_hold_together() {
    let copy = letter(&WORDS[..8]);
    let mut inputs: Vec<_> = (0..2).map(|_| tailored(NOW, &copy)).collect();
    inputs.extend((0..8).map(|_| untailored(NOW)));
    let fired = nudge(&inputs).expect("fires");
    assert_eq!(
        fired.triggers,
        vec![NudgeTrigger::Untailored, NudgeTrigger::NearCopyLetters]
    );
    assert_eq!((fired.letters, fired.near_copy_letters), (2, 2));
}

#[test]
fn ten_tailored_applications_with_no_letters_do_not_fire() {
    let inputs: Vec<_> = (0..10).map(|_| tailored(NOW, "")).collect();
    assert_eq!(nudge(&inputs), None);
}

#[test]
fn applications_older_than_the_window_do_not_pad_the_count() {
    let mut inputs: Vec<_> = (0..9).map(|_| untailored(NOW)).collect();
    inputs.extend((0..20).map(|_| untailored(NOW - 3 * DAY)));
    assert_eq!(nudge(&inputs), None);
}
