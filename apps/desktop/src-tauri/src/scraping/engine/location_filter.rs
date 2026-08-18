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
const REMOTE_MARKERS: &[&str] = &[
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

/// Significant lowercase place-name tokens from the requested location (its city
/// and region text), expanded with any known exonym counterpart (see
/// [`expand_exonyms`]). Empty when nothing usable was requested (e.g. only a
/// country code), which makes the filter inert.
fn requested_needles(requested: &LocationSpec) -> Vec<String> {
    let mut needles: Vec<String> = Vec::new();
    for field in [requested.city.as_deref(), requested.region.as_deref()] {
        let Some(text) = field else { continue };
        for tok in text.split(|c: char| !c.is_alphanumeric()) {
            let tok = tok.trim().to_lowercase();
            if tok.chars().count() >= MIN_TOKEN_LEN && !needles.contains(&tok) {
                needles.push(tok);
            }
        }
    }
    expand_exonyms(&mut needles);
    needles
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
    /// Positive evidence on both sides, and they agree: the posting is remote,
    /// or it names a place the request asked for.
    Match,
    /// Positive evidence on both sides, and they conflict: the posting names a
    /// concrete place, is not remote, and NO requested place token appears in
    /// it. The only answer that is ever a knock-out.
    Mismatch,
    /// No comparison was possible — the posting states no location, or the
    /// request carries no usable place token (country-code-only, or a token
    /// below [`MIN_TOKEN_LEN`]). Never a pass and never a fail.
    Undecided,
}

/// Compare one posting's location text against a requested [`LocationSpec`],
/// three-valued. Conservative, in the same order [`location_mismatch`] has
/// always evaluated:
/// - remote (via the board's `extra.remote` flag OR a remote marker in the
///   location text) → [`LocationVerdict::Match`]
/// - empty / unknown posting location → [`LocationVerdict::Undecided`]
/// - no usable requested place tokens (e.g. country-code-only) →
///   [`LocationVerdict::Undecided`]
/// - a diaeresis-spelling variant of the same name, or a curated exonym (see
///   [`EXONYM_PAIRS`]) → [`LocationVerdict::Match`] (never a false mismatch on
///   München/Munich, Köln/Cologne)
/// - otherwise [`LocationVerdict::Mismatch`], only when NO requested token
///   (fold-aware) appears in the posting location.
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
        return LocationVerdict::Match;
    }
    // Empty / unknown location → the posting states nothing to compare against.
    let loc = match posting_location.map(str::trim) {
        Some(l) if !l.is_empty() => l.to_lowercase(),
        _ => return LocationVerdict::Undecided,
    };
    // Remote marker in the location text.
    if REMOTE_MARKERS.iter().any(|m| loc.contains(m)) {
        return LocationVerdict::Match;
    }
    let needles = requested_needles(requested);
    if needles.is_empty() {
        return LocationVerdict::Undecided; // nothing concrete to match
    }
    // Match when any requested token (diaeresis/exonym-aware) appears in the
    // posting location.
    if needles.iter().any(|n| contains_folded(&loc, n)) {
        LocationVerdict::Match
    } else {
        LocationVerdict::Mismatch
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
mod test {
    use super::*;
    use std::collections::HashMap;

    fn posting(location: Option<&str>, remote: bool) -> JobPosting {
        let mut extra = HashMap::new();
        if remote {
            extra.insert("remote".to_string(), serde_json::json!(true));
        }
        JobPosting {
            id: "b:1".into(),
            external_id: None,
            title: "Engineer".into(),
            company: "Acme".into(),
            location: location.map(str::to_string),
            url: "https://acme.example/1".into(),
            source: "b".into(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: 0,
            extra,
        }
    }

    fn requested(city: &str) -> LocationSpec {
        LocationSpec {
            city: Some(city.into()),
            ..Default::default()
        }
    }

    #[test]
    fn keeps_matching_city() {
        let req = requested("Berlin");
        assert!(!location_mismatch(
            &posting(Some("Berlin, Germany"), false),
            &req
        ));
        assert!(!location_mismatch(
            &posting(Some("Greater Berlin Area"), false),
            &req
        ));
    }

    #[test]
    fn drops_clear_city_mismatch() {
        let req = requested("Berlin");
        assert!(location_mismatch(&posting(Some("London, UK"), false), &req));
        assert!(location_mismatch(&posting(Some("Munich"), false), &req));
    }

    #[test]
    fn keeps_empty_or_unknown_location() {
        let req = requested("Berlin");
        assert!(!location_mismatch(&posting(None, false), &req));
        assert!(!location_mismatch(&posting(Some("   "), false), &req));
    }

    #[test]
    fn keeps_remote_by_flag_or_text() {
        let req = requested("Berlin");
        // Remote flag (Remotive/RemoteOK/WWR) even with a concrete, non-matching
        // location string — never drop a remote job.
        assert!(!location_mismatch(&posting(Some("USA Only"), true), &req));
        // Remote marker in the text, no flag.
        assert!(!location_mismatch(
            &posting(Some("Remote (US)"), false),
            &req
        ));
        assert!(!location_mismatch(&posting(Some("Anywhere"), false), &req));
        assert!(!location_mismatch(
            &posting(Some("Homeoffice, Köln"), false),
            &req
        ));
    }

    #[test]
    fn inert_when_no_usable_place_token() {
        // Country-code-only request (no city text) → no needles → keep everything.
        let cc_only = LocationSpec {
            country_code: Some("de".into()),
            ..Default::default()
        };
        assert!(!location_mismatch(
            &posting(Some("London, UK"), false),
            &cc_only
        ));
        // Too-short city token (< 3 chars) → inert.
        let short = requested("NY");
        assert!(!location_mismatch(&posting(Some("London"), false), &short));
    }

    #[test]
    fn case_insensitive_match() {
        let req = requested("BERLIN");
        assert!(!location_mismatch(&posting(Some("berlin"), false), &req));
    }

    #[test]
    fn conservative_keeps_partial_token_overlap() {
        // Shared token ("san") → kept, erring toward keeping (conservative).
        let req = requested("San Francisco");
        assert!(!location_mismatch(
            &posting(Some("San Diego, CA"), false),
            &req
        ));
    }

    #[test]
    fn filter_postings_counts_drops_and_keeps_order() {
        let req = requested("Berlin");
        let postings = vec![
            posting(Some("Berlin"), false), // keep
            posting(Some("London"), false), // drop
            posting(None, false),           // keep (unknown)
            posting(Some("Remote"), false), // keep (remote text)
            posting(Some("Paris"), false),  // drop
        ];
        let (kept, dropped) = filter_postings(postings, &req);
        assert_eq!(dropped, 2);
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].location.as_deref(), Some("Berlin"));
        assert_eq!(kept[1].location, None);
        assert_eq!(kept[2].location.as_deref(), Some("Remote"));
    }

    #[test]
    fn no_drops_returns_zero() {
        let req = requested("Berlin");
        let postings = vec![posting(Some("Berlin"), false), posting(None, false)];
        let (kept, dropped) = filter_postings(postings, &req);
        assert_eq!(dropped, 0);
        assert_eq!(kept.len(), 2);
    }

    // ── HIGH-2: diaeresis spelling variants (folding) ──────────────────────────

    #[test]
    fn diaeresis_spelling_variants_of_the_same_name_are_never_dropped() {
        // Same word, three spellings — native, DIN-5007-2 transliteration, and
        // bare (diaeresis stripped). Any request spelling must match any posting
        // spelling.
        for (req_city, posting_loc) in [
            ("Köln", "Koeln, Deutschland"),
            ("Koeln", "Köln, Deutschland"),
            ("Koln", "Köln, Deutschland"),
            ("München", "Muenchen"),
            ("Muenchen", "München"),
        ] {
            let req = requested(req_city);
            assert!(
                !location_mismatch(&posting(Some(posting_loc), false), &req),
                "{req_city:?} must match posting {posting_loc:?} (diaeresis spelling variant)"
            );
        }
    }

    // ── HIGH-2: curated exonym pairs ────────────────────────────────────────────

    #[test]
    fn curated_exonym_pairs_are_never_dropped() {
        // The exact false-drop the critic reported: München/Munich, Köln/Cologne,
        // Nürnberg/Nuremberg — bridged only via the curated EXONYM_PAIRS table,
        // NOT by folding (folding cannot turn "munich" into "munchen").
        for (req_city, posting_loc) in [
            ("Munich", "München, Bayern"),
            ("München", "Munich, Germany"),
            ("Cologne", "Köln"),
            ("Köln", "Cologne, Germany"),
            ("Nuremberg", "Nürnberg"),
        ] {
            let req = requested(req_city);
            assert!(
                !location_mismatch(&posting(Some(posting_loc), false), &req),
                "curated exonym: {req_city:?} must not drop posting {posting_loc:?}"
            );
        }
    }

    #[test]
    fn exonym_table_is_folding_aware_on_the_german_side() {
        // A "Munich" request must also accept a transliterated/bare posting
        // spelling of München (Muenchen/Munchen), not just the native form —
        // the exonym expansion and the diaeresis fold compose.
        let req = requested("Munich");
        assert!(!location_mismatch(&posting(Some("Muenchen"), false), &req));
    }

    #[test]
    fn exonym_gap_outside_the_curated_table_is_documented_and_still_drops() {
        // HONESTY CHECK (HIGH-2): the curated table is intentionally bounded to
        // three DACH pairs. An exonym pair NOT in the table (English "The Hague"
        // vs Dutch "Den Haag") is NOT bridged — this pins the documented
        // limitation rather than silently pretending it's fixed. A real, more
        // complete exonym/geocoding table would need to grow this table, not
        // change the matching mechanics.
        let req = requested("Hague");
        assert!(
            location_mismatch(&posting(Some("Den Haag, Netherlands"), false), &req),
            "an exonym pair outside EXONYM_PAIRS is a known, documented gap — must still drop"
        );
    }

    // ── Three-valued verdict (the constraint pass's source) ────────────────────

    /// Every verdict this matcher can return, pinned to an ABSOLUTE expected
    /// value per input — not to whatever `location_mismatch` happens to say.
    #[test]
    fn location_verdict_answers_match_mismatch_and_undecided() {
        let berlin = requested("Berlin");
        // Match: the posting names the requested city.
        assert_eq!(
            location_verdict(Some("Berlin, Germany"), false, &berlin),
            LocationVerdict::Match
        );
        // Match: remote, by the board flag even with a far-away location text.
        assert_eq!(
            location_verdict(Some("Austin, TX"), true, &berlin),
            LocationVerdict::Match
        );
        // Match: remote, by a marker in the location text.
        assert_eq!(
            location_verdict(Some("Remote (US)"), false, &berlin),
            LocationVerdict::Match
        );
        // Mismatch: a concrete, non-remote, non-matching place.
        assert_eq!(
            location_verdict(Some("Austin, TX"), false, &berlin),
            LocationVerdict::Mismatch
        );
        // Undecided: the posting states no location at all.
        assert_eq!(
            location_verdict(None, false, &berlin),
            LocationVerdict::Undecided
        );
        assert_eq!(
            location_verdict(Some("   "), false, &berlin),
            LocationVerdict::Undecided
        );
        // Undecided: nothing usable was requested (country-code-only).
        let cc_only = LocationSpec {
            country_code: Some("de".into()),
            ..Default::default()
        };
        assert_eq!(
            location_verdict(Some("Austin, TX"), false, &cc_only),
            LocationVerdict::Undecided
        );
    }

    /// The scrape-time filter is EXACTLY the `Mismatch` projection — nothing
    /// else drops. Anchored on both ends: a hand-written absolute expectation
    /// per case, plus the equivalence, so a regression that broke both the
    /// filter and the verdict the same way still fails on the absolutes.
    #[test]
    fn location_mismatch_drops_exactly_the_mismatch_verdict() {
        let berlin = requested("Berlin");
        // (posting location, board remote flag, expected verdict, expected drop)
        let cases: &[(Option<&str>, bool, LocationVerdict, bool)] = &[
            (
                Some("Berlin, Germany"),
                false,
                LocationVerdict::Match,
                false,
            ),
            (Some("Austin, TX"), true, LocationVerdict::Match, false),
            (Some("Anywhere"), false, LocationVerdict::Match, false),
            (Some("Austin, TX"), false, LocationVerdict::Mismatch, true),
            (Some("London, UK"), false, LocationVerdict::Mismatch, true),
            (None, false, LocationVerdict::Undecided, false),
            (Some(""), false, LocationVerdict::Undecided, false),
        ];
        for (loc, remote, want_verdict, want_drop) in cases {
            let verdict = location_verdict(*loc, *remote, &berlin);
            assert_eq!(&verdict, want_verdict, "verdict for {loc:?}/{remote}");
            let dropped = location_mismatch(&posting(*loc, *remote), &berlin);
            assert_eq!(dropped, *want_drop, "drop for {loc:?}/{remote}");
            assert_eq!(
                dropped,
                verdict == LocationVerdict::Mismatch,
                "the filter must be exactly the Mismatch projection ({loc:?}/{remote})"
            );
        }
    }

    #[test]
    fn folding_alone_does_not_bridge_an_exonym_without_the_table() {
        // Direct proof that folding is NOT what makes München/Munich match —
        // the fold of "munich" and the fold of "münchen" are simply different
        // strings, confirming the exonym table (not folding) does the bridging.
        let (m_base, m_ue) = fold_variants("münchen");
        let (n_base, n_ue) = fold_variants("munich");
        assert_ne!(m_base, n_base);
        assert_ne!(m_base, n_ue);
        assert_ne!(m_ue, n_base);
        assert_ne!(m_ue, n_ue);
    }
}
