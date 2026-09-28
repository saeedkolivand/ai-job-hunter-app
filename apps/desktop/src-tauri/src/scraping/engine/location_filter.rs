//! Central, conservative location post-filter (trust PR F).
//!
//! Boards that do NOT consume the requested location server-side
//! ([`Scraper::supports_location`](crate::scraping::types::Scraper::supports_location)
//! `== false`) can return postings from anywhere. When the user requested a
//! location, the engine drops the postings whose OWN location CLEARLY mismatches
//! — but conservatively: it never drops a posting with an empty/unknown location
//! or a remote marker, because keeping a wrong-city row is the old lie and
//! dropping a remote job would be a new one. The requested location is matched by
//! significant place-name tokens (case-insensitive substring); a bare country
//! code with no place text leaves the filter inert.

use crate::scraping::types::{JobPosting, LocationSpec};

/// Location-text substrings that mark a posting as location-agnostic (remote).
/// Matched case-insensitively against the posting's location; a hit means
/// "never drop". Covers what the remote boards actually emit
/// (Remotive/RemoteOK/WWR set `extra.remote=true` AND strings like
/// "Worldwide"/"Anywhere"; German feeds emit "Homeoffice").
// `pub(crate)` (issue #1167) — `agent_read::found_jobs`'s `remote` filter reuses this EXACT list
// (never a second hand-typed one) so a found-jobs row is classified "remote" by the identical
// marker set the scrape-time post-filter already uses.
pub(crate) const REMOTE_MARKERS: &[&str] = &[
    "remote",
    "anywhere",
    "worldwide",
    "world wide",
    "home office",
    "home-office",
    "homeoffice",
    "work from home",
    "work-from-home",
    "wfh",
    "distributed",
];

/// Minimum length of a requested place-name token used for matching. Two-letter
/// tokens (a bare country code, "UK") are too noisy to match on, so the filter
/// stays inert unless a real place name is present.
///
/// **This constant now has a SECOND consumer with a different risk posture.**
/// Its original job was noise control for the scrape filter, where being wrong
/// costs one dropped row. The hard-constraint pass
/// (`commands::match_resume::constraints`) publishes a `met` verdict to the user
/// off the same tokens, and the shipped UI location defaults all end in a
/// two-letter qualifier — "San Francisco, CA", "New York, NY", "London, UK".
/// They read [`LocationVerdict::PlaceMatch`] against realistic postings ("San
/// Francisco, California", "New York, United States", "London, United Kingdom")
/// ONLY because 3 drops that qualifier; at 2 each would carry a token the
/// posting does not name and would silently degrade to `unknown`. Lowering this
/// for a scrape-side reason is therefore a user-visible change, and
/// `the_shipped_ui_location_defaults_read_as_a_match` fails if it happens.
const MIN_TOKEN_LEN: usize = 3;

/// A tiny, curated table of English⇄German exonym pairs for the handful of
/// major DACH cities this project's German-market boards
/// (arbeitsagentur/germantechjobs/berlinstartupjobs) actually surface.
///
/// **This is deliberately NOT a general place-name/geocoding database.**
/// [`fold_variants`] fixes SAME-NAME diacritic spelling variants (native
/// "Köln" vs transliterated "Koeln" vs bare "Koln" — one word, three
/// spellings). It can NEVER bridge an exonym pair like Munich/München:
/// those are two different WORDS for the same city, not a spelling variant
/// of one — folding "münchen" and "munich" never produces the same string.
/// The entries below are a bounded, explicit lookup for that separate
/// problem. Any exonym pair NOT in this table is a known, documented gap:
/// an unmatched city name still falls through to the filter's existing
/// (conservative) drop path.
const EXONYM_PAIRS: &[(&str, &str)] = &[
    ("munich", "münchen"),
    ("cologne", "köln"),
    ("nuremberg", "nürnberg"),
];

/// Base-letter and DIN-5007-2-transliteration folds of `s`, lowercased.
/// Real postings and typed requests spell German diaeresis characters
/// interchangeably — native ("München"), transliterated ("Muenchen"), or
/// bare ("Munchen") — so both folds are returned; matching against either
/// bridges all three spellings of the SAME name. Pure.
fn fold_variants(s: &str) -> (String, String) {
    let lower = s.to_lowercase();
    let mut base = String::with_capacity(lower.len());
    let mut translit = String::with_capacity(lower.len() + 4);
    for c in lower.chars() {
        match c {
            'ä' => {
                base.push('a');
                translit.push_str("ae");
            }
            'ö' => {
                base.push('o');
                translit.push_str("oe");
            }
            'ü' => {
                base.push('u');
                translit.push_str("ue");
            }
            'ß' => {
                base.push_str("ss");
                translit.push_str("ss");
            }
            other => {
                base.push(other);
                translit.push(other);
            }
        }
    }
    (base, translit)
}

/// True when `a` and `b` denote the same word once diaeresis spelling is
/// normalised (either fold convention, either side) — e.g. "Köln" == "Koeln"
/// == "Koln". Word-level equality (not substring) so this is precise enough
/// to drive the exonym-table lookup. Pure.
fn folds_equal(a: &str, b: &str) -> bool {
    let (a_base, a_ue) = fold_variants(a);
    let (b_base, b_ue) = fold_variants(b);
    a_base == b_base || a_base == b_ue || a_ue == b_base || a_ue == b_ue
}

/// True when `needle`'s folded form (either convention) appears as a
/// substring of `haystack`'s folded form (either convention) — the
/// diaeresis-spelling-aware version of `haystack.contains(needle)`. Pure.
fn contains_folded(haystack: &str, needle: &str) -> bool {
    let (h_base, h_ue) = fold_variants(haystack);
    let (n_base, n_ue) = fold_variants(needle);
    h_base.contains(&n_base)
        || h_base.contains(&n_ue)
        || h_ue.contains(&n_base)
        || h_ue.contains(&n_ue)
}

/// Expand `needles` in place with the curated [`EXONYM_PAIRS`] table: when a
/// requested token names one side of a known pair, add the OTHER side too —
/// e.g. a "Munich" request also accepts a "München" posting. See
/// [`EXONYM_PAIRS`] for the documented scope limitation. Pure.
fn expand_exonyms(needles: &mut Vec<String>) {
    let originals = needles.clone();
    for tok in &originals {
        for (en, de) in EXONYM_PAIRS {
            if folds_equal(tok, en) && !needles.iter().any(|n| folds_equal(n, de)) {
                needles.push((*de).to_string());
            } else if folds_equal(tok, de) && !needles.iter().any(|n| folds_equal(n, en)) {
                needles.push((*en).to_string());
            }
        }
    }
}

/// Split `text` into lowercase alphanumeric tokens. The one tokenizer both the
/// requested side and the posting side go through, so "Berlin, Germany" cannot
/// be cut two different ways depending on which end it came from.
fn tokenize(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
}

/// The requested location's own significant tokens, deduped, BEFORE any exonym
/// expansion. Split out of [`requested_needles`] so the whole-token matcher can
/// ask "did every token the user actually typed find a home?" — a question the
/// expanded list cannot answer, since expansion adds words the user never wrote.
fn significant_tokens(requested: &LocationSpec) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for field in [requested.city.as_deref(), requested.region.as_deref()] {
        let Some(text) = field else { continue };
        for tok in tokenize(text) {
            if tok.chars().count() >= MIN_TOKEN_LEN && !tokens.contains(&tok) {
                tokens.push(tok);
            }
        }
    }
    tokens
}

/// Significant lowercase place-name tokens from the requested location (its city
/// and region text), expanded with any known exonym counterpart (see
/// [`expand_exonyms`]). Empty when nothing usable was requested (e.g. only a
/// country code), which makes the filter inert.
fn requested_needles(requested: &LocationSpec) -> Vec<String> {
    let mut needles = significant_tokens(requested);
    expand_exonyms(&mut needles);
    needles
}

/// True when EVERY token the user typed appears as a WHOLE token of the
/// posting's location (diaeresis/exonym-aware) — the strict half of the
/// comparison.
///
/// The difference from the substring test [`requested_needles`] feeds is the
/// whole point of having both. Substring, any-token: "San Francisco" matches a
/// "San Diego" posting on the shared `san`, and one token of a two-token request
/// finding a home is enough. That is the right call for a scrape filter, which
/// is deciding whether to DISCARD a row and should err toward keeping. It is the
/// wrong call for a claim published to the user, where "this posting matches
/// where you are looking" would simply be false.
///
/// Exonyms are expanded PER TOKEN rather than over the whole list: expansion
/// adds a word the user never wrote (a "Munich" request gains "münchen"), so
/// requiring every needle in the EXPANDED list to match would fail the exact
/// pair the table exists to bridge. Each requested token is satisfied by itself
/// or by its own counterpart.
fn every_requested_token_is_whole(loc: &str, requested: &LocationSpec) -> bool {
    let posting_tokens: Vec<String> = tokenize(loc).collect();
    let requested_tokens = significant_tokens(requested);
    if requested_tokens.is_empty() || posting_tokens.is_empty() {
        return false;
    }
    requested_tokens.iter().all(|tok| {
        let mut accepted = vec![tok.clone()];
        expand_exonyms(&mut accepted);
        accepted
            .iter()
            .any(|a| posting_tokens.iter().any(|pt| folds_equal(pt, a)))
    })
}

/// What comparing a posting's own location against a requested one actually
/// established. THREE answers, not two: "we could not decide" is a distinct
/// outcome from "they agree", and collapsing the two is how an absent fact
/// becomes a stated one.
///
/// [`location_mismatch`] (the scrape-time post-filter) only ever needed the
/// binary "drop it?" projection of this, and is defined as exactly that below.
/// The hard-constraint pass in `commands::match_resume::constraints` needs all
/// three, because it REPORTS the answer to the user instead of acting on it —
/// and reporting an undecided constraint as a pass (or as a fail) is a lie in
/// either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocationVerdict {
    /// The posting is remote — by the board's flag or a marker in its location
    /// text — so no place comparison applies at all.
    Remote,
    /// Every token the request actually named appears as a WHOLE token of the
    /// posting's location: "Berlin" ↔ "Berlin, Germany", "Munich" ↔ "München"
    /// via the curated table. Strong enough to state to a user.
    PlaceMatch,
    /// Something matched, but the request is NOT fully accounted for. Named for
    /// the incompleteness rather than for a strength, because it covers two
    /// situations of quite different evidential weight and a consumer must not
    /// assume either one:
    ///
    /// - a coincidental SUBSTRING with no whole-token hit at all — "San
    ///   Francisco" against a "San Diego" posting, sharing only `san`. Weak.
    /// - a genuine whole-token hit on PART of a multi-token request — "Berlin,
    ///   Germany" against a bare "Berlin" posting, where `berlin` matches
    ///   exactly and only the country qualifier is unaccounted for. Strong in
    ///   substance, and still not a completed comparison.
    ///
    /// Both are enough to keep a row in a search — the conservative call when
    /// the cost is discarding a job — and neither is enough to tell a user the
    /// posting matches their stated location. They share one variant because
    /// nothing today needs to tell them apart; split it rather than guess if a
    /// consumer ever does.
    PlaceIncomplete,
    /// Positive evidence on both sides, and they conflict: the posting names a
    /// concrete place, is not remote, and NO requested place token appears in
    /// it anywhere.
    Mismatch,
    /// No comparison was possible — the posting states no location, or the
    /// request carries no usable place token (country-code-only, or a token
    /// below [`MIN_TOKEN_LEN`]). Never a pass and never a fail.
    Undecided,
}

/// Compare one posting's location text against a requested [`LocationSpec`].
/// Evaluated in the same order [`location_mismatch`] always has:
/// - remote (via the board's `extra.remote` flag OR a remote marker in the
///   location text) → [`LocationVerdict::Remote`]
/// - empty / unknown posting location → [`LocationVerdict::Undecided`]
/// - no usable requested place tokens (e.g. country-code-only) →
///   [`LocationVerdict::Undecided`]
/// - no requested token anywhere in the posting location →
///   [`LocationVerdict::Mismatch`]
/// - otherwise something matched, and the last step grades it: every requested
///   token present as a WHOLE token (diaeresis-spelling variants and curated
///   exonyms included, so never a false miss on München/Munich, Köln/Cologne)
///   → [`LocationVerdict::PlaceMatch`]; a mere substring or partial-request hit
///   → [`LocationVerdict::PlaceIncomplete`].
///
/// The last split is the only thing the grading adds, and it exists because the
/// two callers are asking different questions. `location_mismatch` wants "may I
/// discard this row?" and treats overlap as a keep, unchanged. The constraint
/// pass wants "may I tell the user this posting matches where they are looking?"
/// and treats overlap as unknown.
///
/// Takes the two posting facts as plain data rather than a [`JobPosting`] so the
/// L3 constraint pass — which reads a cached posting as `serde_json::Value`, not
/// as a `JobPosting` — reaches the SAME matcher instead of forking a second one
/// that would drift on the remote-marker list and the exonym table.
///
/// Pure — the truth table is unit-tested below.
pub(crate) fn location_verdict(
    posting_location: Option<&str>,
    board_remote: bool,
    requested: &LocationSpec,
) -> LocationVerdict {
    // A posting a board flagged remote can never conflict with a place.
    if board_remote {
        return LocationVerdict::Remote;
    }
    // Empty / unknown location → the posting states nothing to compare against.
    let loc = match posting_location.map(str::trim) {
        Some(l) if !l.is_empty() => l.to_lowercase(),
        _ => return LocationVerdict::Undecided,
    };
    // Remote marker in the location text.
    if REMOTE_MARKERS.iter().any(|m| loc.contains(m)) {
        return LocationVerdict::Remote;
    }
    let needles = requested_needles(requested);
    if needles.is_empty() {
        return LocationVerdict::Undecided; // nothing concrete to match
    }
    // Nothing in common at all.
    if !needles.iter().any(|n| contains_folded(&loc, n)) {
        return LocationVerdict::Mismatch;
    }
    // Something matched. HOW WELL is a separate question, and the two callers
    // need different answers to it — see [`every_requested_token_is_whole`].
    if every_requested_token_is_whole(&loc, requested) {
        LocationVerdict::PlaceMatch
    } else {
        LocationVerdict::PlaceIncomplete
    }
}

/// True when `posting` should be DROPPED for a search that requested `requested`
/// from a board that does not filter location server-side.
///
/// The binary projection of [`location_verdict`]: **only** an explicit
/// [`LocationVerdict::Mismatch`] drops. Every other answer — including every
/// undecided one — keeps, which is the conservative posture this filter has
/// always had (keeping a wrong-city row is the old lie; dropping a remote or
/// unknown-location job would be a new one). Defined in terms of the verdict so
/// the two callers cannot drift into two different remote-marker lists.
pub(crate) fn location_mismatch(posting: &JobPosting, requested: &LocationSpec) -> bool {
    let board_remote = posting
        .extra
        .get("remote")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    matches!(
        location_verdict(posting.location.as_deref(), board_remote, requested),
        LocationVerdict::Mismatch
    )
}

/// Drop postings whose location clearly mismatches `requested`, returning the
/// kept postings (in input order) and the number dropped. Pure — see
/// [`location_mismatch`].
pub(crate) fn filter_postings(
    postings: Vec<JobPosting>,
    requested: &LocationSpec,
) -> (Vec<JobPosting>, usize) {
    let before = postings.len();
    let kept: Vec<JobPosting> = postings
        .into_iter()
        .filter(|p| !location_mismatch(p, requested))
        .collect();
    let dropped = before - kept.len();
    (kept, dropped)
}

#[cfg(test)]
mod test;
