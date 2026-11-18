//! Fixtures shared by the insights tests. Literal timestamps, no clock.

use crate::applications::{StatusEvent, EVENT_SOURCE_USER};
use crate::insights::{AppInput, GenerationFacts, Outcome};

/// One day in ms, spelled out so no test derives its expectation from the constant under test.
pub(super) const DAY: u64 = 86_400_000;
/// An arbitrary "now" (2026-11-17 in ms), far from any month boundary.
pub(super) const NOW: u64 = 1_794_000_000_000;

pub(super) fn event(to: &str, source: &str, confirmed: bool) -> StatusEvent {
    StatusEvent {
        event_id: 0,
        application_id: "a".into(),
        from_status: String::new(),
        to_status: to.into(),
        at: 0,
        note: String::new(),
        source: source.into(),
        confirmed,
    }
}

pub(super) fn user_event(to: &str) -> StatusEvent {
    event(to, EVENT_SOURCE_USER, true)
}

pub(super) fn generation(lang: &str, template: Option<&str>, letter: &str) -> GenerationFacts {
    GenerationFacts {
        target_language: lang.into(),
        template: template.map(str::to_string),
        cover_letter: letter.into(),
    }
}

pub(super) fn input(
    applied_at: u64,
    board: &str,
    outcome: Option<Outcome>,
    generation: Option<GenerationFacts>,
) -> AppInput {
    AppInput {
        applied_at,
        board: board.into(),
        outcome,
        generation,
    }
}

/// `n` applications on `board` with `outcome`, no generation, applied at `at`.
pub(super) fn many(n: usize, board: &str, outcome: Outcome, at: u64) -> Vec<AppInput> {
    (0..n)
        .map(|_| input(at, board, Some(outcome), None))
        .collect()
}
