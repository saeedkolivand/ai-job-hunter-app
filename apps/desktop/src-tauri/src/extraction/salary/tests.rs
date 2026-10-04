use super::*;

/// Every currency/separator/suffix shape the heuristic accepts, matched verbatim.
#[test]
fn matches_each_supported_range_shape() {
    let cases: &[(&str, &str)] = &[
        // dollar range with thousands separators and a period suffix
        (
            "Salary: $50,000 - $70,000 per year",
            "$50,000 - $70,000 per year",
        ),
        // euro range with no spaces
        ("Compensation: €40,000-€60,000", "€40,000-€60,000"),
        // pound range with the `to` separator
        ("Pay: £30,000 to £45,000", "£30,000 to £45,000"),
        // ISO-code range
        ("Band: USD 50,000 - USD 70,000", "USD 50,000 - USD 70,000"),
        // `k` suffix
        ("We pay $80k-$120k", "$80k-$120k"),
        // en-dash separator
        ("$50,000 \u{2013} $70,000", "$50,000 \u{2013} $70,000"),
        // per-hour suffix
        ("$25-$35 per hour", "$25-$35 per hour"),
        // per-year slash suffix
        ("€40,000-€60,000/year", "€40,000-€60,000/year"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            extract_salary_range(input).as_deref(),
            Some(*expected),
            "input: {input:?}"
        );
    }
}

/// Text the conservative heuristic must never read as a salary range: a single number, a date or
/// year range, a percentage, `401k` alone, or no salary text at all.
#[test]
fn does_not_match_non_salary_text() {
    for input in [
        // a single number
        "Salary: $75,000",
        // a date range
        "Contract term: 2020 - 2021",
        "Employment: Jan 2020 - Dec 2021",
        // a percentage
        "Annual bonus of 10-15%",
        // a currency-prefixed number followed by a percentage
        "Base salary $60,000 - 10% commission on top",
        "Compensation: $50,000-15% equity vesting",
        "Base $60,000 to 10% signing bonus",
        // 401k alone
        "We offer a 401k retirement match",
        // no salary text present
        "We are looking for a Senior Rust Engineer with 5 years experience.",
        // A bare year range: no currency on either side — neither branch can fire. Both the
        // en-dash (issue repro) and hyphen spellings are covered.
        "Contract term: 2024 – 2025",
        "Contract term: 2024 - 2025",
        // "10.000" is a NUMBER and "Schritte" follows it, but "Schritte" (steps) is not a
        // currency — the trailing branch's required post-number currency rejects it
        // (issue #1222's "10.000 Schritte" negative).
        "Täglich 10.000 Schritte gehen",
    ] {
        assert!(extract_salary_range(input).is_none(), "input: {input:?}");
    }
}

#[test]
fn normalizes_internal_whitespace() {
    let got = extract_salary_range("$50,000   -\n  $70,000").unwrap();
    assert_eq!(got, "$50,000 - $70,000");
}

#[test]
fn caps_the_returned_string_length() {
    // The clamp is a belt-and-braces cap independent of what the regex can produce —
    // exercised directly so a future looser pattern can't silently exceed it.
    let long = "$".to_string() + &"1".repeat(500);
    assert!(clamp(long).len() <= MAX_SALARY_FACT_LEN);
}

// ── ungrouped-bound / truncation fix (the "$120,000 -> $120" defect) ────────
// Table-driven: every case must resolve the SAME way regardless of whether the number is
// thousands-grouped, ungrouped, or spaced — never a truncated range.

#[test]
fn ungrouped_and_truncation_cases() {
    let cases: &[(&str, Option<&str>)] = &[
        // One side grouped, the other ungrouped — must match IN FULL, not truncate at "$120".
        ("Salary: $100,000-$120000", Some("$100,000-$120000")),
        // Both sides ungrouped — must match in full.
        ("Salary: $100000-$120000", Some("$100000-$120000")),
        // Grouped with spaces around the separator — must still match in full (regression).
        ("Salary: $100,000 - $120,000", Some("$100,000 - $120,000")),
        // A value followed by more digits than the bounded ungrouped run can hold (10 digits) —
        // the candidate is cut short, so it must be rejected outright, not returned truncated.
        ("Salary: $50,000-$1234567890 annually", None),
    ];
    for (input, expected) in cases {
        assert_eq!(
            extract_salary_range(input).as_deref(),
            *expected,
            "input: {input:?}"
        );
    }
}

#[test]
fn is_truncated_continuation_detects_digit_comma_and_period_adjacency() {
    assert!(is_truncated_continuation("000 more text"));
    assert!(is_truncated_continuation(",000 more text"));
    assert!(is_truncated_continuation(".5 more text"));
    assert!(!is_truncated_continuation(" more text"));
    assert!(!is_truncated_continuation("% commission"));
    assert!(!is_truncated_continuation(""));
}

// ── trailing-currency (German) shape + magnitude floor (issue #1222) ───────

#[test]
fn matches_german_trailing_currency_range() {
    // The exact issue #1222 repro: German postings put the currency AFTER each number,
    // and the German thousands separator is the `.` (60.000 = 60,000).
    assert_eq!(
        extract_salary_range("Gehalt: 60.000 € – 75.000 €").as_deref(),
        Some("60.000 € – 75.000 €")
    );
    // A trailing word after the range must not prevent the match.
    assert_eq!(
        extract_salary_range("Gehalt: 60.000 € – 75.000 € brutto").as_deref(),
        Some("60.000 € – 75.000 €")
    );
}

#[test]
fn rejects_ranges_too_small_to_be_a_salary() {
    // Both endpoints far below the floor — numeric noise, not a salary range.
    assert!(extract_salary_range("Honorar: 3 € – 5 €").is_none());
    // A small lower bound is fine as long as the LARGER endpoint clears the floor.
    assert_eq!(
        extract_salary_range("Honorar: 9.000 € – 12.000 €").as_deref(),
        Some("9.000 € – 12.000 €")
    );
}

#[test]
fn per_hour_ranges_are_exempt_from_the_floor() {
    // The floor must NOT swallow real per-hour ranges whose endpoints are small on
    // purpose — "$25 - $35 per hour" keeps matching. The same magnitude with no
    // period exercises the floor where it applies, the TRAILING branch, and is
    // rejected there as noise ("25 € – 35 €"). (A leading-currency "$25-$35" with
    // no period is NOT floored — see `leading_currency_ranges_are_never_floored`.)
    assert_eq!(
        extract_salary_range("$25-$35 per hour").as_deref(),
        Some("$25-$35 per hour")
    );
    assert!(extract_salary_range("Honorar: 25 € – 35 €").is_none());
}

#[test]
fn leading_currency_ranges_are_never_floored() {
    // Round-2 no-regression guard: the floor is scoped to the trailing-currency
    // branch and must not regress the pre-existing leading-currency path. These are
    // ordinary sub-10k MONTHLY ranges whose period words the conservative
    // `PERIOD_SUFFIX` never recognizes ("a month" is neither `/` nor `per `, "pro
    // Monat" is German) — flooring the leading branch silently dropped them. HEAD
    // matched all three byte-for-byte; so must we.
    assert_eq!(
        extract_salary_range("Salary: £2,500 - £3,500 a month").as_deref(),
        Some("£2,500 - £3,500")
    );
    assert_eq!(
        extract_salary_range("Gehalt: €4.500 - €6.000 pro Monat").as_deref(),
        Some("€4.500 - €6.000")
    );
    // No period at all: a small leading-currency range is still never floored
    // (HEAD returned "$25-$35" too).
    assert_eq!(extract_salary_range("$25-$35").as_deref(), Some("$25-$35"));
}
