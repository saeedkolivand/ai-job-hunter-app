//! Contact fields read from the header block: email, phone, location, and the
//! zone of lines they are searched in.

use std::sync::LazyLock;

use regex::Regex;

use super::dates::find_range;
use super::text::split_name_prefix;
use super::{AtsField, AtsReason};
use crate::documents::evidence::GEOGRAPHY_TOKENS;

use super::super::EMAIL_RE;

pub(super) fn find_email(lines: &[String]) -> (AtsField, Option<usize>) {
    lines
        .iter()
        .enumerate()
        .find_map(|(i, l)| {
            EMAIL_RE
                .find(l)
                .map(|m| (AtsField::found(m.as_str()), Some(i)))
        })
        .unwrap_or_else(|| (AtsField::missing(), None))
}

static PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\+?\(?\d[\d \t().\-/]{5,}\d").unwrap());

/// The contact zone: every line before the first heading plus the lines
/// around the email line (a sidebar template extracts them after the body).
/// Lines are returned in document order; a fused name prefix is dropped.
pub(super) fn contact_zone(
    lines: &[String],
    first_heading: Option<usize>,
    email_idx: Option<usize>,
    name_idx: Option<usize>,
) -> Vec<String> {
    let header_end = first_heading.unwrap_or(lines.len());
    let near_email = email_idx.map(|e| e.saturating_sub(1)..=e + 1);
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < header_end || near_email.as_ref().is_some_and(|r| r.contains(i)))
        .map(|(i, l)| match split_name_prefix(l) {
            Some((_, rest)) if Some(i) == name_idx || i == 0 => rest.to_string(),
            _ => l.clone(),
        })
        .collect()
}

fn digit_count(s: &str) -> usize {
    s.chars().filter(char::is_ascii_digit).count()
}

/// A date range's digits are not a phone number: blank them out first.
fn blank_dates(line: &str) -> String {
    let mut text = line.to_string();
    while let Some(r) = find_range(&text) {
        text.replace_range(r.span.clone(), &" ".repeat(r.span.len()));
    }
    text
}

/// The short digit tail a wrapped number continues with on `next` (`6789 | …`):
/// 2-4 digits at the start of the line that are NOT the start of a word, so a
/// postcode + city (`10115 Berlin`) is never a tail.
fn wrapped_tail(next: &str) -> Option<String> {
    let next = next.trim_start();
    let run: String = next
        .chars()
        .take_while(|c| c.is_ascii_digit() || " \t().-".contains(*c))
        .collect();
    let after_is_word = next[run.len()..]
        .trim_start()
        .chars()
        .next()
        .is_some_and(char::is_alphabetic);
    ((2..=4).contains(&digit_count(&run)) && !after_is_word).then(|| run.trim().to_string())
}

/// The phone number, in document order. Matching is per line; a number that ENDS a
/// line in a phone-like head (`+49 151 2345`) and continues with a short digit tail
/// on the next line is reported `suspicious` (`wrapped`), otherwise a complete match
/// is `found`. `Musterstraße 12` / `10115 Berlin` is never read as a number.
pub(super) fn find_phone(zone: &[String]) -> AtsField {
    let lines: Vec<String> = zone.iter().map(|l| blank_dates(l)).collect();
    for (i, l) in lines.iter().enumerate() {
        for m in PHONE_RE.find_iter(l) {
            let head = m.as_str().trim();
            let dh = digit_count(head);
            if l[m.end()..].trim().is_empty() {
                let wrapped = lines.get(i + 1).and_then(|n| wrapped_tail(n));
                if let Some(tail) = wrapped {
                    let phone_head = head.starts_with(['+', '(']) || dh >= 6;
                    let total = dh + digit_count(&tail);
                    if phone_head && (7..=15).contains(&total) {
                        let value = format!("{head} {tail}");
                        return AtsField::suspicious(Some(value), AtsReason::Wrapped);
                    }
                }
            }
            if (7..=15).contains(&dh) {
                let value = head.split_whitespace().collect::<Vec<_>>().join(" ");
                return AtsField::found(value);
            }
        }
    }
    AtsField::missing()
}

static POSTAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4,5}\s+\p{Lu}[\p{L}. -]+$").unwrap());

fn tokens_are_geography(seg: &str) -> bool {
    let mut tokens = seg
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .peekable();
    tokens.peek().is_some() && tokens.all(|t| GEOGRAPHY_TOKENS.contains(&t.as_str()))
}

/// `City, Country` / `City, ST`: 2-3 comma parts of 1-3 capitalised words.
fn city_comma_region(seg: &str) -> bool {
    let parts: Vec<&str> = seg.split(',').map(str::trim).collect();
    (2..=3).contains(&parts.len())
        && parts.iter().all(|p| {
            let words: Vec<&str> = p.split_whitespace().collect();
            (1..=3).contains(&words.len())
                && words.iter().all(|w| {
                    w.chars().next().is_some_and(char::is_uppercase)
                        && w.chars().all(|c| c.is_alphabetic() || c == '.')
                })
        })
}

pub(super) fn find_location(zone: &[String]) -> AtsField {
    let split = |l: &String| -> Vec<String> {
        l.split(['|', '·', '•', '\t', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| (2..=60).contains(&s.chars().count()))
            .collect()
    };
    let plausible = |s: &str| {
        !s.contains('@')
            && !s.contains("://")
            && !["www.", ".com", ".de", ".io", "linkedin", "github"]
                .iter()
                .any(|u| s.to_lowercase().contains(u))
    };
    let segments: Vec<(String, bool)> = zone
        .iter()
        .flat_map(|l| {
            let multi = l.contains(['|', '·', '•', '@']);
            split(l).into_iter().map(move |s| (s, multi))
        })
        .filter(|(s, _)| plausible(s))
        .collect();
    let hit = segments
        .iter()
        .find(|(s, _)| tokens_are_geography(s) || POSTAL_RE.is_match(s))
        .or_else(|| {
            segments
                .iter()
                .find(|(s, multi)| *multi && city_comma_region(s))
        });
    hit.map_or_else(AtsField::missing, |(s, _)| AtsField::found(s))
}
