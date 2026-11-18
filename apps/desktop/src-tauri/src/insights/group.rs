//! Count and rate outcomes per dimension. Pure.

use std::collections::BTreeMap;

use chrono::DateTime;

use super::{
    AppInput, Dimension, Outcome, OutcomeCounts, OutcomeGroup, OutcomeRates, MAX_GROUP_KEY_BYTES,
    MIN_GROUP_SAMPLE, UNKNOWN_KEY,
};

/// Hex chars of the SHA-256 kept in a shortened key.
const DIGEST_HEX: usize = 16;

fn or_unknown(s: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let s = s.trim();
    if s.is_empty() {
        return UNKNOWN_KEY.to_string();
    }
    if s.len() <= MAX_GROUP_KEY_BYTES {
        return s.to_string();
    }
    // Board and language come from stored or synced text. A plain cut would merge two
    // long values that differ only past it, so an over-cap value becomes a bounded
    // prefix (on a char boundary) plus a digest of the FULL value; the key stays bounded.
    let digest = Sha256::digest(s.as_bytes())
        .iter()
        .take(DIGEST_HEX / 2)
        .fold(String::with_capacity(DIGEST_HEX), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        });
    let end = (0..=MAX_GROUP_KEY_BYTES - DIGEST_HEX - 1)
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    format!("{}#{digest}", &s[..end])
}

fn month_key(ms: u64) -> String {
    i64::try_from(ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map_or_else(
            || UNKNOWN_KEY.to_string(),
            |d| d.format("%Y-%m").to_string(),
        )
}

/// Every `(dimension, key)` slice one application falls into.
fn keys(app: &AppInput) -> [(Dimension, String); 6] {
    let generation = app.generation.as_ref();
    let letter = generation.is_some_and(|g| !g.cover_letter.trim().is_empty());
    [
        (Dimension::Board, or_unknown(&app.board)),
        (
            Dimension::Tailored,
            if generation.is_some() {
                "tailored"
            } else {
                "untailored"
            }
            .to_string(),
        ),
        (
            Dimension::CoverLetter,
            if letter { "included" } else { "none" }.to_string(),
        ),
        (
            Dimension::TargetLanguage,
            or_unknown(generation.map_or("", |g| g.target_language.as_str())),
        ),
        (Dimension::Month, month_key(app.applied_at)),
        (
            Dimension::Template,
            or_unknown(generation.and_then(|g| g.template.as_deref()).unwrap_or("")),
        ),
    ]
}

fn rates(c: &OutcomeCounts) -> Option<OutcomeRates> {
    let decided = c.positive_reply + c.rejected + c.no_reply;
    if decided < MIN_GROUP_SAMPLE {
        return None;
    }
    let frac = |n: u32| f64::from(n) / f64::from(decided);
    Some(OutcomeRates {
        positive_reply: frac(c.positive_reply),
        rejected: frac(c.rejected),
        no_reply: frac(c.no_reply),
    })
}

/// All groups, ordered by dimension then key (months therefore ascend).
pub(super) fn groups(inputs: &[AppInput]) -> Vec<OutcomeGroup> {
    let mut by_slice: BTreeMap<(Dimension, String), OutcomeCounts> = BTreeMap::new();
    for app in inputs {
        let Some(outcome) = app.outcome else { continue };
        for slice in keys(app) {
            let c = by_slice.entry(slice).or_default();
            match outcome {
                Outcome::PositiveReply => c.positive_reply += 1,
                Outcome::Rejected => c.rejected += 1,
                Outcome::NoReply => c.no_reply += 1,
                Outcome::Pending => c.pending += 1,
            }
        }
    }
    by_slice
        .into_iter()
        .map(|((dimension, key), counts)| OutcomeGroup {
            dimension,
            key,
            rates: rates(&counts),
            counts,
        })
        .collect()
}
