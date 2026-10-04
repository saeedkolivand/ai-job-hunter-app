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
//! suffix, with an optional trailing per-hour/per-year period. The currency may LEAD the range —
//! the classic "$50,000 - $70,000" — or TRAIL each number, the German convention
//! ("60.000 € – 75.000 €", issue #1222). Requiring ONE currency marker is what keeps this from
//! ever matching a bare date range ("2020 - 2021"), a bare percentage ("10-15%"), or "401k"
//! alone (no second number/separator) — under-claim over mis-claim, the same discipline as every
//! other extraction in this codebase. A magnitude floor ([`MIN_SALARY_VALUE`]) additionally
//! rejects trailing-currency noise whose larger endpoint can't be a salary ("3 € – 5 €",
//! issue #1222), while trailing ranges carrying a per-hour/per-year period are exempt ("25 € –
//! 35 € per hour" is a real range with small, intentional endpoints). The pre-existing
//! leading-currency branch is never floored — "£2,500 - £3,500 a month" is an ordinary
//! sub-10k MONTHLY range that must keep matching (round-2 scoping fix; see
//! [`is_below_salary_floor`]).

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

/// The smallest LARGER endpoint that can plausibly be an annual full-time salary in any currency
/// this module recognizes — a full-time minimum wage annualizes well above this in every one of
/// them (the German statutory minimum alone is ≈ 25,800 €/yr). Ranges below it are numeric noise
/// ("3 € – 5 €"), not salaries. Applies ONLY to the trailing-currency branch (see
/// [`is_below_salary_floor`]) — the leading branch predates the floor and is never floored, so
/// ordinary sub-10k monthly ranges ("£2,500 - £3,500 a month") still match. Exemption, on the
/// trailing branch: a match carrying the per-hour/per-year period ([`PERIOD_SUFFIX`]) is never
/// floored — "25 € – 35 € per hour" is a real range whose endpoints are small on purpose, and
/// the period is exactly the text that says so.
const MIN_SALARY_VALUE: f64 = 10_000.0;

/// Two branches, alternated at each scan position — the currency LEADING the range (the classic
/// "$50,000 - $70,000", second-side currency optional) or TRAILING each number (the German
/// "60.000 € – 75.000 €", issue #1222). The split is mutually exclusive by construction: the
/// leading branch needs a currency BEFORE the first number (so it can never absorb the German
/// shape, where the first number has no prefix) and the trailing branch needs a currency AFTER
/// the first number (so it can never absorb a bare year range "2024 – 2025" — no currency at
/// all — nor "10 € – 15 €" preceded by plain digits). Both hand the two raw numbers to named
/// captures (`lead_a`/`lead_b`, `trail_a`/`trail_b`) and the optional period suffix to `period`,
/// so the Rust side can apply the magnitude floor to the trailing branch only, and then only
/// when no period is present. Digit groups
/// are individually bounded (see [`NUMBER`]) and each position's match attempt is a small
/// fixed-cost alternation — a linear scan, never catastrophic backtracking.
static SALARY_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)(?:(?:{CURRENCY})\s*(?P<lead_a>{NUMBER})\s*{SEPARATOR}\s*(?:{CURRENCY})?\s*(?P<lead_b>{NUMBER})|(?P<trail_a>{NUMBER})\s*(?:{CURRENCY})\s*{SEPARATOR}\s*(?P<trail_b>{NUMBER})\s*(?:{CURRENCY})?)(?P<period>\s*{PERIOD_SUFFIX})?"
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

/// Parse a bounded salary figure (as produced by [`NUMBER`]) to a `f64` for the magnitude floor:
/// strip commas and thousands-separator dots (a `.` glued to exactly 3 digits at the token end —
/// the German `60.000` convention, `/60.000/` → 60_000) while keeping a trailing `.d`/`.dd`
/// decimal, then apply the `k`/`K` suffix. Never panics: the regex only feeds bounded
/// digit-or-separator runs, and a parse failure (unreachable) falls back to `0.0`.
fn salary_number_value(s: &str) -> f64 {
    let (digits, scale) = match s.strip_suffix(['k', 'K']) {
        Some(rest) => (rest, 1_000.0),
        None => (s, 1.0),
    };
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b',' {
            i += 1;
            continue;
        }
        if b == b'.' {
            let rest = &bytes[i + 1..];
            if rest.len() == 3 && rest.iter().all(u8::is_ascii_digit) {
                i += 1; // thousands-separator dot — drop it
                continue;
            }
        }
        out.push(b as char);
        i += 1;
    }
    out.parse::<f64>().unwrap_or(0.0) * scale
}

/// The two endpoint values of a candidate range, from whichever branch matched.
fn range_endpoints(caps: &regex::Captures<'_>) -> Option<(f64, f64)> {
    if let (Some(a), Some(b)) = (caps.name("lead_a"), caps.name("lead_b")) {
        Some((
            salary_number_value(a.as_str()),
            salary_number_value(b.as_str()),
        ))
    } else {
        let (a, b) = (caps.name("trail_a"), caps.name("trail_b"));
        Some((
            salary_number_value(a?.as_str()),
            salary_number_value(b?.as_str()),
        ))
    }
}

/// True when the range's larger endpoint is below [`MIN_SALARY_VALUE`] — numeric noise ("3 € –
/// 5 €"), not a salary. Applies ONLY to the trailing-currency branch: a leading-currency match
/// (`lead_a`/`lead_b` fired) is the pre-existing HEAD behavior and is never floored — re-flooring
/// it silently dropped ordinary sub-10k MONTHLY ranges ("£2,500 - £3,500 a month") whose period
/// words the conservative [`PERIOD_SUFFIX`] never recognizes (round-2 scoping fix). On the
/// trailing branch, skipped when the match carries a per-hour/per-year period
/// ([`PERIOD_SUFFIX`]): "25 € – 35 € per hour" is exempt by design, and the period is exactly the
/// text that says so. `None` from [`range_endpoints`] (impossible by construction — one branch
/// always fires) is treated as below the floor: reject rather than fabricate a range.
fn is_below_salary_floor(caps: &regex::Captures<'_>) -> bool {
    if caps.name("lead_a").is_some() || caps.name("lead_b").is_some() {
        return false; // leading-currency branch is never floored
    }
    if caps.name("period").is_some() {
        return false;
    }
    match range_endpoints(caps) {
        Some((a, b)) => a.max(b) < MIN_SALARY_VALUE,
        None => true,
    }
}

/// Find the first candidate salary RANGE in `text`, normalized for whitespace only. `None` when
/// nothing matches the conservative heuristic above — this function never guesses, never infers a
/// single number as a range, and never returns anything but the matched substring verbatim. The
/// `regex` crate has no lookahead, so three conditions are rejected here as post-match checks,
/// trying the next candidate instead of fabricating a bad range:
/// - a candidate immediately followed (modulo whitespace) by `%` is a percentage, not a salary
///   range (e.g. "$60,000 - 10% commission");
/// - a candidate whose match end is immediately followed by more digits (see
///   [`is_truncated_continuation`]) means [`NUMBER`]'s bounded groups cut the real number short
///   (e.g. "$100,000-$120000" must never yield "$100,000-$120");
/// - a trailing-currency candidate whose larger endpoint is below [`MIN_SALARY_VALUE`] and has no
///   period suffix is numeric noise, not a range (e.g. "3 € – 5 €", issue #1222; see
///   [`is_below_salary_floor`] — the leading-currency branch is never floored).
pub fn extract_salary_range(text: &str) -> Option<String> {
    SALARY_RANGE_RE
        .captures_iter(text)
        .find(|caps| {
            let range = caps
                .get(0)
                .expect("capture 0 is the whole match — always present");
            let rest = &text[range.end()..];
            !rest.trim_start().starts_with('%')
                && !is_truncated_continuation(rest)
                && !is_below_salary_floor(caps)
        })
        .map(|caps| {
            let range = caps
                .get(0)
                .expect("capture 0 is the whole match — always present");
            clamp(normalize_whitespace(range.as_str()))
        })
}

#[cfg(test)]
mod tests;
