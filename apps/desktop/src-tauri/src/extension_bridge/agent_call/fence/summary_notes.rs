//! The one allowlist for `BoardScrapeSummary.notes` on the agent surface. `notes` is persisted,
//! and a crafted backup import can put any string there, so it is checked on the way out instead
//! of trusted. Both routes use this: the generic tier's shape fencer (`autopilot_get`/`_list`,
//! `jobs_get`) and the `automations` projection.

use serde_json::{Map, Value};

/// The engine's own note producers, each `<token>:<short value>` (a count or a country code):
/// `ats_partial_note`/`ats_failed_fetches_note` (`scraping/boards/common.rs`), Adzuna's
/// `guessed-market`/`broadened`, and the engine's two post-filters (`scrape_boards.rs`).
const KNOWN_NOTE_TOKENS: [&str; 7] = [
    "slugs-invalid",
    "rows-dropped",
    "companies-failed",
    "guessed-market",
    "broadened",
    "location-filtered",
    "work-type-filtered",
];

/// Only `<known token>:<1 to 8 alphanumerics>` passes.
pub(in crate::extension_bridge) fn is_known_note(note: &str) -> bool {
    note.split_once(':').is_some_and(|(token, value)| {
        KNOWN_NOTE_TOKENS.contains(&token)
            && !value.is_empty()
            && value.len() <= 8
            && value.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// Drops every entry of `map["notes"]` that is not [`is_known_note`]. A non-array `notes` is
/// removed outright: nothing but a list of strings is ever a valid producer output.
pub(super) fn retain_known_notes(map: &mut Map<String, Value>) {
    match map.get_mut("notes") {
        Some(Value::Array(items)) => {
            items.retain(|n| n.as_str().is_some_and(is_known_note));
        }
        Some(_) => {
            map.remove("notes");
        }
        None => {}
    }
}
