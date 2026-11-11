//! The long-dash ban: the one definition of "a clause dash", shared by
//! `voice.em_dash_overuse` (which counts them) and the deterministic strip the
//! pipeline applies after humanize (which removes them). The prompt's
//! "EM-DASH HARD BAN" rule is what instructs the model; this is what enforces
//! it when the model ignores the rule.

/// The em and en dash.
const LONG_DASHES: [char; 2] = ['—', '–'];

/// Month and weekday names (English, German; full and short): a spaced dash
/// between two of them is a range ("March – June", "Mon – Fri"), not a clause.
const RANGE_NAMES: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
    "januar",
    "februar",
    "märz",
    "mai",
    "juni",
    "juli",
    "oktober",
    "dezember",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "mon",
    "tue",
    "wed",
    "thu",
    "fri",
    "sat",
    "sun",
    "montag",
    "dienstag",
    "mittwoch",
    "donnerstag",
    "freitag",
    "samstag",
    "sonntag",
    "mo",
    "di",
    "mi",
    "do",
    "fr",
    "sa",
    "so",
];

/// A neighbouring word that makes the dash a range: it carries a digit or names
/// a month or weekday.
fn range_word(word: Option<&str>) -> bool {
    word.is_some_and(|w| {
        let w = w
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        w.chars().any(|c| c.is_ascii_digit()) || RANGE_NAMES.contains(&w.as_str())
    })
}

/// An unspaced em dash between letters is a clause break only when neither side
/// is a single short fragment or a range name, and at least one side is a
/// lowercase word (two capitalised words are a name).
fn clause_break(chars: &[char], i: usize) -> bool {
    let left: String = chars[..i]
        .iter()
        .rev()
        .take_while(|c| c.is_alphabetic())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let right: String = chars[i + 1..]
        .iter()
        .take_while(|c| c.is_alphabetic())
        .collect();
    let named = |w: &str| RANGE_NAMES.contains(&w.to_lowercase().as_str());
    let lowercase = |w: &str| w.starts_with(char::is_lowercase);
    left.chars().count() >= 2
        && right.chars().count() >= 2
        && !named(&left)
        && !named(&right)
        && (lowercase(&left) || lowercase(&right))
}

/// Whether the dash at `i` is a clause dash the ban applies to: spaces on both
/// sides, text on both sides, not a leading list marker, and not part of a
/// date, month or weekday range. An unspaced dash (`Rolls–Royce`, `Jan–Mar`)
/// is never touched.
fn is_prose_dash(chars: &[char], i: usize, first: Option<usize>) -> bool {
    if Some(i) == first || i == 0 || i + 1 >= chars.len() {
        return false;
    }
    // The most common AI tell: an unspaced EM dash that reads as a clause break
    // ("fast—and reliable"). Never an unspaced en dash (`Rolls–Royce`,
    // `Jan–Mar`), never one beside a digit (`2019—2022`), and not a name, a
    // range or a split word (`Rolls—Royce`, `Jan—Mar`, `e—mail`).
    if chars[i] == '—' && chars[i - 1].is_alphabetic() && chars[i + 1].is_alphabetic() {
        return clause_break(chars, i);
    }
    if !chars[i - 1].is_whitespace() || !chars[i + 1].is_whitespace() {
        return false;
    }
    let before: String = chars[..i].iter().collect();
    let after: String = chars[i + 1..].iter().collect();
    if after.trim().is_empty() {
        return false;
    }
    !range_word(before.split_whitespace().last()) && !range_word(after.split_whitespace().next())
}

/// A line the ban never touches or counts: it carries a URL.
fn is_url_line(line: &str) -> bool {
    line.contains("://") || line.contains("www.")
}

/// How many clause dashes `strip_long_dashes` would remove: the same predicate, so a dash the
/// strip deliberately keeps (`Rolls–Royce`, `Jan–Mar`, a date range) never counts as overuse.
pub(super) fn clause_dash_count(text: &str) -> usize {
    text.split('\n')
        .filter(|line| !is_url_line(line))
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let first = chars.iter().position(|c| !c.is_whitespace());
            (0..chars.len())
                .filter(|&i| LONG_DASHES.contains(&chars[i]) && is_prose_dash(&chars, i, first))
                .count()
        })
        .sum()
}

/// `text` with every prose clause dash replaced by a comma.
///
/// Conservative by construction: a dash is only replaced when it is spaced on
/// both sides, or is an unspaced em dash between two letters, and sits between
/// ordinary words ([`is_prose_dash`]), and a line
/// carrying a URL is left whole, so a date column, a range, a compound name or
/// a bullet marker can never be altered. Deterministic and idempotent.
pub(crate) fn strip_long_dashes(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if !line.contains(LONG_DASHES) || is_url_line(line) {
            out.push(line.to_string());
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let first = chars.iter().position(|c| !c.is_whitespace());
        let mut rebuilt = String::new();
        let mut i = 0;
        while i < chars.len() {
            if LONG_DASHES.contains(&chars[i]) && is_prose_dash(&chars, i, first) {
                rebuilt.truncate(rebuilt.trim_end().len());
                rebuilt.push_str(", ");
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                continue;
            }
            rebuilt.push(chars[i]);
            i += 1;
        }
        out.push(rebuilt);
    }
    out.join("\n")
}
