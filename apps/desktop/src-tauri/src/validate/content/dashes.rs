//! The long-dash ban: the one definition of "a clause dash", shared by
//! `voice.em_dash_overuse` (which counts them) and the deterministic strip the
//! pipeline applies after humanize (which removes them). The prompt's
//! "EM-DASH HARD BAN" rule is what instructs the model; this is what enforces
//! it when the model ignores the rule.

/// The em and en dash.
const LONG_DASHES: [char; 2] = ['—', '–'];

/// A dash with a digit beside it (`2020 – 2023`, `2021 – Present`): a date or
/// numeric range, correct typography.
fn digit_adjacent(chars: &[char], i: usize) -> bool {
    let before = chars[..i].iter().rev().find(|c| !c.is_whitespace());
    let after = chars[i + 1..].iter().find(|c| !c.is_whitespace());
    before.is_some_and(char::is_ascii_digit) || after.is_some_and(char::is_ascii_digit)
}

/// A tight numeric range: the validator's own exemption (`2020–2023`).
fn tight_range(chars: &[char], i: usize) -> bool {
    i.checked_sub(1)
        .and_then(|p| chars.get(p))
        .is_some_and(char::is_ascii_digit)
        && chars.get(i + 1).is_some_and(char::is_ascii_digit)
}

/// How many long dashes sit between clauses (tight numeric ranges excluded).
pub(super) fn clause_dash_count(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .filter(|&i| LONG_DASHES.contains(&chars[i]) && !tight_range(&chars, i))
        .count()
}

/// `text` with every prose dash replaced by a comma (or dropped at a line edge).
///
/// Prose only: a dash with a digit beside it is a date or range and stays, and a
/// line carrying a URL is left whole, so neither a date column nor a link can be
/// altered. Deterministic and idempotent.
pub(crate) fn strip_long_dashes(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if !line.contains(LONG_DASHES) || line.contains("://") || line.contains("www.") {
            out.push(line.to_string());
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut rebuilt = String::new();
        let mut skip_space = false;
        for (i, &c) in chars.iter().enumerate() {
            if LONG_DASHES.contains(&c) && !digit_adjacent(&chars, i) {
                let trimmed = rebuilt.trim_end().len();
                rebuilt.truncate(trimmed);
                let at_end = chars[i + 1..].iter().all(|c| c.is_whitespace());
                if !rebuilt.trim().is_empty() && !at_end {
                    rebuilt.push(',');
                    rebuilt.push(' ');
                }
                skip_space = true;
                continue;
            }
            if skip_space && c.is_whitespace() {
                continue;
            }
            skip_space = false;
            rebuilt.push(c);
        }
        out.push(rebuilt);
    }
    out.join("\n")
}
