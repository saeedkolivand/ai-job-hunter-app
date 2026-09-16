//! Pure salary-range extraction from a job-posting text blob — `match.live`'s `salary.posting`
//! fact (PR3, design decision 5 / R4). Deliberately narrow: this module ONLY finds a candidate
//! salary RANGE substring and normalizes its whitespace — it never guesses, never infers a single
//! number as a range, and never produces any comparison/verdict. The `expectation` half of the
//! wire pair (`JobPreferences.salary_expectation`) is assembled by `extension_bridge::match_live`,
//! not here — this module knows nothing about the user's own preferences, only the posting text.
//!
//! Lives in `extraction` (L1) rather than `extension_bridge` (L3) because it is pure
//! text-extraction domain logic, not wire mapping — `extension_bridge::match_live` calls
//! [`extract_salary_range`] and does only the IPC/wire mapping itself.
//!
//! ## The heuristic (conservative, not exhaustive)
//! A currency SYMBOL or ISO CODE, adjacent (modulo whitespace) to two numbers separated by a
//! dash/en-dash/em-dash/"to", each number optionally carrying thousands separators and a `k`/`K`
//! suffix, with an optional trailing per-hour/per-year period. Requiring the currency marker is
//! what keeps this from ever matching a bare date range ("2020 - 2021"), a bare percentage
//! ("10-15%"), or "401k" alone (no second number/separator) — under-claim over mis-claim, the same
//! discipline as every other extraction in this codebase.

use std::sync::LazyLock;

use regex::Regex;

/// Belt-and-braces cap on the returned matched substring — the pattern's own bounded digit groups
/// (see [`NUMBER`]) already keep a real match well under this.
const MAX_SALARY_FACT_LEN: usize = 80;

const CURRENCY: &str = r"(?:[$€£¥₹]|\b(?:USD|EUR|GBP|JPY|CAD|AUD|CHF|INR|SEK|NOK|DKK|PLN)\b)";
/// Two branches: thousands-GROUPED (`\d{1,3}` then 1-3 `,ddd`/`.ddd` groups — up to 3 groups is
/// already far past any real salary figure) OR a bounded UNGROUPED run of up to 9 raw digits (same
/// rationale — no real salary needs more). Grouped requires >=1 separator group so it only fires
/// on text that actually has thousands separators; an ungrouped number like `120000` fails that
/// branch (no comma/period to feed a group) and falls through to the ungrouped one instead, which
/// consumes it whole. Both branches are still individually bounded, so a match can never run away —
/// but the boundary between "matched everything" and "matched only the bounded prefix and left more
/// digits dangling" still needs a Rust-side check (`regex` has no lookahead): see
/// [`is_truncated_continuation`].
const NUMBER: &str = r"(?:\d{1,3}(?:[,.]\d{3}){1,3}|\d{1,9})(?:\.\d{1,2})?[kK]?";
const SEPARATOR: &str = r"(?:-|–|—|\bto\b)";
const PERIOD_SUFFIX: &str = r"(?:/|per\s+)?(?:hour|hr|year|yr|annum|month|mo)\.?";

static SALARY_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i){CURRENCY}\s*{NUMBER}\s*{SEPARATOR}\s*{CURRENCY}?\s*{NUMBER}(?:\s*{PERIOD_SUFFIX})?"
    ))
    .expect("salary range regex must compile — pattern is a fixed literal")
});

/// Collapse internal whitespace runs to single spaces and trim — the ONLY normalization applied;
/// the matched text is otherwise shown byte-for-byte, verbatim, per the design's "two facts, never
/// a judgement" rule.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate `s` to at most [`MAX_SALARY_FACT_LEN`] bytes, cutting on a UTF-8 char boundary — same
/// discipline as `job_preferences::clamp_bytes`.
fn clamp(mut s: String) -> String {
    if s.len() <= MAX_SALARY_FACT_LEN {
        return s;
    }
    let mut end = MAX_SALARY_FACT_LEN;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// True when `rest` (the text immediately following a candidate match's end) starts with a digit,
/// or a comma/period immediately followed by a digit — i.e. [`NUMBER`]'s bounded digit groups cut
/// the real number short rather than the number genuinely ending there. Deliberately NOT
/// whitespace-tolerant (unlike the `%` check below): a truncated continuation is always digit-
/// adjacent with no space, since a space would start a new token (a unit, a word), not more of the
/// same number.
fn is_truncated_continuation(rest: &str) -> bool {
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some(',') | Some('.') => matches!(chars.next(), Some(d) if d.is_ascii_digit()),
        _ => false,
    }
}

/// Find the first candidate salary RANGE in `text`, normalized for whitespace only. `None` when
/// nothing matches the conservative heuristic above — this function never guesses, never infers a
/// single number as a range, and never returns anything but the matched substring verbatim. The
/// `regex` crate has no lookahead, so two conditions are rejected here as post-match checks, trying
/// the next candidate instead of fabricating a bad range: a candidate immediately followed (modulo
/// whitespace) by `%` is a percentage, not a salary range (e.g. "$60,000 - 10% commission"); a
/// candidate whose match end is immediately followed by more digits (see
/// [`is_truncated_continuation`]) means [`NUMBER`]'s bounded groups cut the real number short (e.g.
/// "$100,000-$120000" must never yield "$100,000-$120").
pub fn extract_salary_range(text: &str) -> Option<String> {
    SALARY_RANGE_RE
        .find_iter(text)
        .find(|m| {
            let rest = &text[m.end()..];
            !rest.trim_start().starts_with('%') && !is_truncated_continuation(rest)
        })
        .map(|m| clamp(normalize_whitespace(m.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_dollar_range_with_thousands_separators_and_period_suffix() {
        assert_eq!(
            extract_salary_range("Salary: $50,000 - $70,000 per year").as_deref(),
            Some("$50,000 - $70,000 per year")
        );
    }

    #[test]
    fn matches_a_euro_range_with_no_spaces() {
        assert_eq!(
            extract_salary_range("Compensation: €40,000-€60,000").as_deref(),
            Some("€40,000-€60,000")
        );
    }

    #[test]
    fn matches_a_pound_range_with_to_separator() {
        assert_eq!(
            extract_salary_range("Pay: £30,000 to £45,000").as_deref(),
            Some("£30,000 to £45,000")
        );
    }

    #[test]
    fn matches_an_iso_code_range() {
        assert_eq!(
            extract_salary_range("Band: USD 50,000 - USD 70,000").as_deref(),
            Some("USD 50,000 - USD 70,000")
        );
    }

    #[test]
    fn matches_k_suffix_ranges() {
        assert_eq!(
            extract_salary_range("We pay $80k-$120k").as_deref(),
            Some("$80k-$120k")
        );
    }

    #[test]
    fn matches_en_dash_separator() {
        assert_eq!(
            extract_salary_range("$50,000 \u{2013} $70,000").as_deref(),
            Some("$50,000 \u{2013} $70,000")
        );
    }

    #[test]
    fn matches_per_hour_suffix() {
        assert_eq!(
            extract_salary_range("$25-$35 per hour").as_deref(),
            Some("$25-$35 per hour")
        );
    }

    #[test]
    fn matches_per_year_slash_suffix() {
        assert_eq!(
            extract_salary_range("€40,000-€60,000/year").as_deref(),
            Some("€40,000-€60,000/year")
        );
    }

    #[test]
    fn does_not_match_a_single_number() {
        assert!(extract_salary_range("Salary: $75,000").is_none());
    }

    #[test]
    fn does_not_match_a_date_range() {
        assert!(extract_salary_range("Contract term: 2020 - 2021").is_none());
        assert!(extract_salary_range("Employment: Jan 2020 - Dec 2021").is_none());
    }

    #[test]
    fn does_not_match_a_percentage() {
        assert!(extract_salary_range("Annual bonus of 10-15%").is_none());
    }

    #[test]
    fn does_not_match_a_currency_prefixed_number_followed_by_a_percentage() {
        assert!(extract_salary_range("Base salary $60,000 - 10% commission on top").is_none());
        assert!(extract_salary_range("Compensation: $50,000-15% equity vesting").is_none());
        assert!(extract_salary_range("Base $60,000 to 10% signing bonus").is_none());
    }

    #[test]
    fn does_not_match_401k_alone() {
        assert!(extract_salary_range("We offer a 401k retirement match").is_none());
    }

    #[test]
    fn does_not_match_when_no_salary_text_present() {
        assert!(extract_salary_range(
            "We are looking for a Senior Rust Engineer with 5 years experience."
        )
        .is_none());
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
}
