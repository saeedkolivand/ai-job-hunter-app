use super::super::*;

// ── Date-filter mapping helpers ───────────────────────────────────────────────

#[test]
fn adzuna_max_days_old_maps_correctly() {
    // All sub-day windows FLOOR at 3 days: Adzuna has no sub-day granularity, and a
    // 1-day ceiling zeroed out autopilot "recent" filters on quiet days (regression
    // guard). Date-sort still surfaces the freshest jobs first within the window.
    assert_eq!(adzuna_max_days_old(Some("24h")), 3);
    assert_eq!(adzuna_max_days_old(Some("8h")), 3);
    assert_eq!(adzuna_max_days_old(Some("4h")), 3);
    assert_eq!(adzuna_max_days_old(Some("2h")), 3);
    assert_eq!(adzuna_max_days_old(Some("1h")), 3);
    assert_eq!(adzuna_max_days_old(Some("30m")), 3);
    assert_eq!(adzuna_max_days_old(Some("15m")), 3);
    // Coarser tiers are unchanged.
    assert_eq!(adzuna_max_days_old(Some("week")), 7);
    assert_eq!(adzuna_max_days_old(Some("month")), 30);
    // No filter or an unknown token caps at the past month (30 days).
    assert_eq!(adzuna_max_days_old(None), 30);
    assert_eq!(adzuna_max_days_old(Some("99y")), 30);
}

#[test]
fn adzuna_where_keeps_first_segment_and_drops_country_suffix() {
    // Redundant country suffix (either language) is dropped — the country is
    // already the URL path segment, so keeping it over-narrows the geocode.
    assert_eq!(adzuna_where("Köln, Deutschland"), "Köln");
    assert_eq!(adzuna_where("Cologne, Germany"), "Cologne");
    // A country-name-only location already returns Adzuna's full page: pass through.
    assert_eq!(adzuna_where("germany"), "germany");
    // Empty stays empty (caller treats "" as country-wide).
    assert_eq!(adzuna_where(""), "");
    // Surrounding + inner whitespace is trimmed off the kept segment.
    assert_eq!(adzuna_where("  Berlin , Germany "), "Berlin");
}

#[test]
fn should_broaden_only_for_explicit_sparse_market() {
    // Explicit country + non-empty where + count under the floor (3) → broaden.
    assert!(should_broaden(false, "Köln", 2));

    // Regression guard: a GUESSED market must never broaden — it would defeat
    // primary_chain's guessed-market → JSearch fallback, which relies on Adzuna
    // returning empty for a probably-wrong guess.
    assert!(!should_broaden(true, "Köln", 0));

    // Empty `where` means "already country-wide" — nothing left to broaden to.
    assert!(!should_broaden(false, "", 0));

    // At/above the floor, the result isn't sparse — don't retry.
    assert!(!should_broaden(false, "Köln", 3));
}

#[test]
fn guessed_market_note_only_for_authoritative_guess() {
    // Guessed market + real location + an AUTHORITATIVE result (>= the floor of 3)
    // → surface the otherwise-silent market guess, carrying the country code only.
    assert_eq!(
        guessed_market_note(true, "London", 3, "de").as_deref(),
        Some("guessed-market:de")
    );
    assert_eq!(
        guessed_market_note(true, "London", 12, "us").as_deref(),
        Some("guessed-market:us"),
        "the note carries the guessed market's country code, never the raw location"
    );

    // Guessed market but a SUB-floor result (0/1/2): primary_chain re-routes to the
    // global fallback, so NO guessed-market results are actually shown — flagging
    // the guess here would mislead. No note.
    for count in [0usize, 1, 2] {
        assert_eq!(
            guessed_market_note(true, "London", count, "de"),
            None,
            "a sub-floor guessed result ({count}) routes to the fallback — no note"
        );
    }

    // Explicit country (not guessed) → never a guessed-market note; the broaden
    // path owns the sparse-city case for an explicitly-supplied market.
    assert_eq!(guessed_market_note(false, "London", 10, "de"), None);

    // Guessed market but EMPTY / whitespace-only location → guessing a default
    // market for a location-less browse is expected, not worth surfacing.
    assert_eq!(guessed_market_note(true, "", 10, "de"), None);
    assert_eq!(guessed_market_note(true, "   ", 10, "de"), None);
}

#[test]
fn jsearch_date_posted_maps_correctly() {
    // All sub-day windows floor at "3days" (JSearch has no sub-day token, and
    // "today" zeroed out autopilot "recent" filters on quiet days — regression guard).
    assert_eq!(jsearch_date_posted(Some("24h")), "3days");
    assert_eq!(jsearch_date_posted(Some("8h")), "3days");
    assert_eq!(jsearch_date_posted(Some("4h")), "3days");
    assert_eq!(jsearch_date_posted(Some("2h")), "3days");
    assert_eq!(jsearch_date_posted(Some("1h")), "3days");
    assert_eq!(jsearch_date_posted(Some("30m")), "3days");
    assert_eq!(jsearch_date_posted(Some("15m")), "3days");
    // Coarser tiers are unchanged.
    assert_eq!(jsearch_date_posted(Some("week")), "week");
    assert_eq!(jsearch_date_posted(Some("month")), "month");
    // No filter or an unknown token caps at the past month.
    assert_eq!(jsearch_date_posted(None), "month");
    assert_eq!(jsearch_date_posted(Some("99y")), "month");
}

// ── Date-filter exhaustiveness: every generated TS token is handled ────────────
//
// `DATE_FILTER_OPTIONS` is the codegen'd mirror of the TS `DATE_FILTER_OPTIONS`
// (the single source of truth in `packages/shared/src/schemas/index.ts`). Both
// match arms fall through to a DEFAULT for an unknown token, so a NEW TS token
// would silently collapse to that default instead of getting a real mapping.
//
// This test pins the EXPECTED non-default mapping for every known token and
// iterates the generated list, asserting each token maps to its expected value
// for BOTH `adzuna_max_days_old` and `jsearch_date_posted`. A new TS token added
// without a Rust match arm (or without an entry here) FAILS this test, so the
// cross-language drift surfaces at `cargo test` rather than at runtime.

/// Expected `(adzuna_max_days_old, jsearch_date_posted)` for a known token, or
/// `None` if the token is unrecognised (which must FAIL — every generated token
/// is required to have a real, non-default mapping).
fn expected_mapping(token: &str) -> Option<(u32, &'static str)> {
    match token {
        // Sub-day windows floor at 3 days ("3days" for JSearch) — see the doc-comments
        // on `adzuna_max_days_old` / `jsearch_date_posted`. A tighter clamp zeroed out
        // autopilot "recent" filters; date-sort keeps the freshest jobs on top.
        "15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h" => Some((3, "3days")),
        "week" => Some((7, "week")),
        "month" => Some((30, "month")),
        _ => None,
    }
}

/// The single token whose mapping is INTENDED to equal the no-filter default
/// pair (`adzuna_max_days_old(None)`, `jsearch_date_posted(None)`). Any OTHER
/// token collapsing to that pair is the silent-default bug this guard catches.
const INTENDED_DEFAULT_EQUAL_TOKEN: &str = "month";

#[test]
fn every_generated_date_filter_token_has_a_real_mapping() {
    // The no-filter default pair, read from the SAME mappers (not a literal), so
    // this guard tracks the real defaults even if they ever change.
    let default_pair = (adzuna_max_days_old(None), jsearch_date_posted(None));

    for &token in crate::ipc_contracts::date_filters::DATE_FILTER_OPTIONS {
        let (exp_days, exp_posted) = expected_mapping(token).unwrap_or_else(|| {
            panic!(
                "generated date-filter token {token:?} has no expected mapping — a new TS token \
                 was added without a Rust match arm in `adzuna_max_days_old` / `jsearch_date_posted`"
            )
        });

        assert_eq!(
            adzuna_max_days_old(Some(token)),
            exp_days,
            "adzuna_max_days_old({token:?}) must map to its expected value, not the default"
        );
        assert_eq!(
            jsearch_date_posted(Some(token)),
            exp_posted,
            "jsearch_date_posted({token:?}) must map to its expected value, not the default"
        );

        // Companion guard: a token whose mapping equals BOTH no-filter defaults at
        // once has silently collapsed to the default in both arms. That is only
        // legitimate for the one documented default-equal token; any other token
        // doing so (e.g. a future token given a default-equal expected mapping by
        // mistake) FAILS here even though its expected-mapping assertion passed.
        let token_pair = (
            adzuna_max_days_old(Some(token)),
            jsearch_date_posted(Some(token)),
        );
        if token != INTENDED_DEFAULT_EQUAL_TOKEN {
            assert_ne!(
                token_pair, default_pair,
                "date-filter token {token:?} maps to the no-filter default pair {default_pair:?} \
                 in BOTH arms — it has silently collapsed to the default instead of getting a real \
                 mapping (only {INTENDED_DEFAULT_EQUAL_TOKEN:?} may equal the default pair)"
            );
        }
    }
}
