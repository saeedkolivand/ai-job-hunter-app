//! The volume nudge: many applications in the last day, mostly untailored or
//! sent with near-identical letters. Pure; derived on read, nothing stored.

use std::collections::HashSet;

use super::{
    AppInput, NudgeTrigger, VolumeNudge, HOUR_MS, MAX_NUDGE_LETTERS, NEAR_COPY_JACCARD,
    NUDGE_MAJORITY_SHARE, NUDGE_MIN_APPLICATIONS, NUDGE_WINDOW_HOURS,
};
use crate::documents::keywords::keywords_normalized_for_lang;
use crate::validate::content::jaccard;

/// How many of `tokens` have a partner at or above [`NEAR_COPY_JACCARD`].
fn near_copies(tokens: &[HashSet<String>]) -> usize {
    (0..tokens.len())
        .filter(|&i| {
            (0..tokens.len())
                .any(|j| j != i && jaccard(&tokens[i], &tokens[j]) >= NEAR_COPY_JACCARD)
        })
        .count()
}

fn over_majority(part: usize, whole: usize) -> bool {
    whole > 0 && part as f64 / whole as f64 > NUDGE_MAJORITY_SHARE
}

/// The nudge for the trailing window ending at `now`, or `None` when it does not fire.
pub(super) fn volume_nudge(inputs: &[AppInput], now: u64) -> Option<VolumeNudge> {
    let window: Vec<&AppInput> = inputs
        .iter()
        .filter(|a| a.applied_at <= now && now - a.applied_at <= NUDGE_WINDOW_HOURS * HOUR_MS)
        .collect();
    let applications = window.len();
    if applications < NUDGE_MIN_APPLICATIONS as usize {
        return None;
    }
    let untailored = window.iter().filter(|a| a.generation.is_none()).count();
    let mut letters: Vec<(&str, &str)> = window
        .iter()
        .filter_map(|a| a.generation.as_ref())
        .filter(|g| !g.cover_letter.trim().is_empty())
        .map(|g| (g.cover_letter.as_str(), g.target_language.as_str()))
        .collect();
    letters.truncate(MAX_NUDGE_LETTERS);
    // The compared set: `letters` and `near_copy_letters` describe the same letters.
    let letter_count = letters.len();
    let tokens: Vec<HashSet<String>> = letters
        .iter()
        .map(|(text, lang)| keywords_normalized_for_lang(text, lang))
        .collect();
    let near_copy_letters = near_copies(&tokens);

    let mut triggers = Vec::new();
    if over_majority(untailored, applications) {
        triggers.push(NudgeTrigger::Untailored);
    }
    if over_majority(near_copy_letters, tokens.len()) {
        triggers.push(NudgeTrigger::NearCopyLetters);
    }
    if triggers.is_empty() {
        return None;
    }
    let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
    Some(VolumeNudge {
        window_hours: u32::try_from(NUDGE_WINDOW_HOURS).unwrap_or(u32::MAX),
        applications: n(applications),
        untailored: n(untailored),
        letters: n(letter_count),
        near_copy_letters: n(near_copy_letters),
        triggers,
    })
}
