use super::{body_fetch_item_spec, build_search_query, uid_sequence_set, MAX_BODY_BYTES};
use chrono::NaiveDate;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

// ── body_fetch_item_spec (/review MEDIUM: protocol-level body bound) ────

#[test]
fn body_fetch_item_spec_is_bounded_by_a_partial_octet_range() {
    // Pins the EXACT wire spec — a bare `BODY.PEEK[]` regression (fetching
    // an unbounded whole message again) would fail this, not just a
    // "contains a number somewhere" check.
    assert_eq!(
        body_fetch_item_spec(),
        format!("(UID BODY.PEEK[]<0.{MAX_BODY_BYTES}>)")
    );
    assert_eq!(body_fetch_item_spec(), "(UID BODY.PEEK[]<0.200000>)");
}

// ── build_search_query (/review MEDIUM: watermark-scoped SEARCH) ────────

#[test]
fn build_search_query_cases() {
    let cases = [
        // matching uidvalidity AND a stored watermark: bound by uid
        (true, Some(100), "UID 101:* SINCE 16-Jul-2026".to_string()),
        // No watermark yet (first-ever connect) — nothing to bound by, even
        // though uidvalidity trivially "matches" (both `None`/absent).
        (true, None, "SINCE 16-Jul-2026".to_string()),
        // A stale watermark from a DIFFERENT uidvalidity generation must
        // never be used to bound the search, even if one is stored.
        (false, Some(100), "SINCE 16-Jul-2026".to_string()),
        // never overflows at the uid ceiling
        (
            true,
            Some(u32::MAX),
            format!("UID {}:* SINCE 16-Jul-2026", u32::MAX),
        ),
    ];
    for (uidvalidity_matches, stored_last_uid, expected) in cases {
        assert_eq!(
            build_search_query(date(2026, 7, 16), uidvalidity_matches, stored_last_uid),
            expected,
            "uidvalidity_matches={uidvalidity_matches}, stored_last_uid={stored_last_uid:?}"
        );
    }
}

#[test]
fn uid_sequence_set_cases() {
    let cases: [(&[u32], &str); 3] = [
        (&[101, 102, 105], "101,102,105"),
        // a single uid has no comma
        (&[42], "42"),
        // Callers (`fetch_bodies`) short-circuit before this is ever built
        // with an empty slice, but the pure fn itself stays total.
        (&[], ""),
    ];
    for (uids, expected) in cases {
        assert_eq!(uid_sequence_set(uids), expected, "{uids:?}");
    }
}
