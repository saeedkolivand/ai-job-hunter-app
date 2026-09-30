//! Shared test fixtures: the `Result` deserialization target and the
//! `extract_json` helper (pins `candidates`'s pre-parse extraction order).

use serde::Deserialize;

use super::super::*;

#[derive(Debug, Deserialize, PartialEq)]
pub(super) struct Result {
    pub(super) score: u8,
    pub(super) notes: String,
}

/// The JSON value [`parse`] tries FIRST — [`candidates`]'s head, which is
/// the pre-hardening "commit to one extraction" behavior.
///
/// A TEST-ONLY helper: this used to be a `pub fn extract_json` on the
/// crate's forward surface with zero callers and a doc telling callers not
/// to use it — a trap that hands a caller the exact behavior HIGH-1 was
/// about (committing to a span before serde has had a say). The assertions
/// below are worth keeping because they pin [`candidates`]'s ORDER, so the
/// helper moved in here with them.
///
/// String-aware — a brace or bracket inside a JSON string never opens or
/// closes a region, which the two parsers [`parse`] replaces both got wrong
/// (a `"note": "use {} here"` value truncated the extraction). `None` when
/// nothing opens, and also when something opens but never closes
/// ([`parse`] tells those two apart).
pub(super) fn extract_json(raw: &str) -> Option<&str> {
    candidates(raw).into_iter().next()
}
