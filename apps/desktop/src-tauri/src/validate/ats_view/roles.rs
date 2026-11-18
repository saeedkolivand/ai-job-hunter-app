//! Per-role extraction from the experience section(s) of the re-extracted text.
//!
//! A role is anchored on a line carrying a date range. Title and company come
//! from that line and the one below it, the way a rigid parser reads a
//! single-column CV (`Company  dates` / `Title`, `Title | Company | dates`, or
//! `dates Company Title` for a left-label layout). Nothing here guesses a
//! vocabulary of job titles: a field that cannot be told apart is reported
//! `suspicious`, never invented.

use super::dates::{end_value, find_range, point_value, End, Range2, Ym};
use super::text::Heading;
use super::{AtsField, AtsReason, AtsRole, AtsSectionKind};
use crate::documents::evidence::split_entry;
use crate::export::types::{LineKind, ParsedLine};

/// Glyphs a bullet line starts with, or that fuse a bullet onto a title line.
const BULLETS: [char; 6] = ['•', '▪', '●', '◦', '·', '■'];

/// Longest line that can still be a job title or company name.
const MAX_LABEL_CHARS: usize = 80;

pub(super) fn is_bullet(c: char) -> bool {
    BULLETS.contains(&c)
}

fn before_bullet(line: &str) -> &str {
    line.split(BULLETS).next().unwrap_or(line).trim()
}

fn starts_with_bullet(line: &str) -> bool {
    line.starts_with(BULLETS) || line.starts_with("- ")
}

/// `rest` with the date range cut out, trimmed of separators.
fn without_range(line: &str, range: &Range2) -> String {
    let joined = format!("{} {}", &line[..range.span.start], &line[range.span.end..]);
    joined
        .trim_matches(|c: char| c.is_whitespace() || "|·,;–—-()".contains(c))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Acme CorpSenior Engineer`: two glyph runs pdf-extract joined with no space
/// (`pdftotext` separates them on the same Lebenslauf render, so this is an
/// extractor artifact, not a template defect — the split values are reported as
/// found). Only used when the line below offers no title, so a company such as
/// `McKinsey` on a two-line entry is never split.
fn split_fused(s: &str) -> Option<(&str, &str)> {
    let mut prev: Option<char> = None;
    for (i, c) in s.char_indices() {
        let next_lower = s[i..].chars().nth(1).is_some_and(char::is_lowercase);
        if i > 0 && prev.is_some_and(char::is_alphabetic) && c.is_uppercase() && next_lower {
            let (a, b) = s.split_at(i);
            return (!a.contains(['|', ',']) && b.contains(' ')).then_some((a, b));
        }
        prev = Some(c);
    }
    None
}

fn label_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().count() <= MAX_LABEL_CHARS && !s.ends_with(['.', ':'])
}

/// `a` is earlier than `b`: by year, and by month only when BOTH carry one
/// (`Mar 2021 – 2021` is not inverted).
fn ends_before(a: Ym, b: Ym) -> bool {
    a.year < b.year
        || (a.year == b.year && matches!((a.month, b.month), (Some(x), Some(y)) if x < y))
}

/// A line is a role anchor only if what is left after the date range reads like a
/// label, not prose: a wrapped bullet continuation (`… between 2019 – 2021 across
/// the platform migration …`) must not open a phantom role. An open-ended range
/// (`seit 2020`, `from 2019`) is also the start of ordinary sentences, so it needs a
/// short capitalised remainder.
fn is_role_anchor(head: &str, range: &Range2) -> bool {
    let rest = without_range(head, range);
    if rest.is_empty() {
        return true;
    }
    let open_ended = range.to_raw.is_empty();
    label_ok(&rest)
        && (!open_ended
            || (rest.split_whitespace().count() <= 5
                && rest.chars().next().is_some_and(char::is_uppercase)))
}

fn date_fields(range: &Range2) -> (AtsField, AtsField) {
    let unparsed = |raw: &str| AtsField::suspicious(Some(raw.to_string()), AtsReason::Unparseable);
    let start = range.from.map_or_else(
        || unparsed(&range.from_raw),
        |p| AtsField::found(point_value(p)),
    );
    let mut end = match range.to {
        Some(e) => AtsField::found(end_value(e)),
        None => unparsed(&range.to_raw),
    };
    if let (Some(from), Some(End::Date(to))) = (range.from, range.to) {
        if ends_before(to, from) {
            end = AtsField::suspicious(Some(end_value(End::Date(to))), AtsReason::DatesInverted);
        }
    }
    (start, end)
}

/// `Title | Company`, `Title — Company`, `Title at Company`, `Title, Company`.
fn split_label(label: &str) -> (Option<String>, Option<String>) {
    for sep in ["|", "·", " – ", " — ", " at ", " bei ", ","] {
        if let Some((title, company)) = label.split_once(sep) {
            let (t, c) = (title.trim(), company.trim());
            if !t.is_empty() && !c.is_empty() {
                return (Some(t.to_string()), Some(c.to_string()));
            }
        }
    }
    (Some(label.to_string()), None)
}

fn entry_line(text: &str) -> ParsedLine {
    ParsedLine {
        kind: LineKind::JobEntry,
        raw: text.to_string(),
        text: text.to_string(),
        segments: Vec::new(),
        right_text: None,
    }
}

fn field(v: Option<String>) -> AtsField {
    v.filter(|s| !s.is_empty())
        .map_or_else(AtsField::missing, AtsField::found)
}

fn role_for(lines: &[String], i: usize, range: &Range2, below: Option<&str>) -> AtsRole {
    let head = before_bullet(&lines[i]);
    let rest = without_range(head, range);
    let (start, end) = date_fields(range);
    let below = below.map(before_bullet).filter(|b| label_ok(b));

    let (title, company) = if rest.is_empty() {
        // `dates` alone on the line: the label is the line below.
        let (t, c) = below.map(split_label).unwrap_or((None, None));
        (field(t), field(c))
    } else if rest.contains(['|', '·']) {
        let (company, title, _) = split_entry(&entry_line(&rest));
        (field(Some(title)), field(Some(company)))
    } else if let Some(t) = below {
        (field(Some(t.to_string())), field(Some(rest)))
    } else if let Some((company, title)) = split_fused(&rest) {
        (
            field(Some(title.to_string())),
            field(Some(company.to_string())),
        )
    } else {
        (AtsField::missing(), field(Some(rest)))
    };
    AtsRole {
        title,
        company,
        start,
        end,
    }
}

/// Every role found in the experience section(s) of `lines`.
pub(super) fn extract_roles(lines: &[String], headings: &[Heading]) -> Vec<AtsRole> {
    let mut roles = Vec::new();
    for (n, h) in headings.iter().enumerate() {
        if h.kind != AtsSectionKind::Experience {
            continue;
        }
        let end = headings.get(n + 1).map_or(lines.len(), |next| next.idx);
        let anchors: Vec<(usize, Range2)> = (h.idx + 1..end)
            .filter(|&i| !starts_with_bullet(&lines[i]))
            .filter_map(|i| {
                let head = before_bullet(&lines[i]);
                find_range(head)
                    .filter(|r| is_role_anchor(head, r))
                    .map(|r| (i, r))
            })
            .collect();
        for (k, (i, range)) in anchors.iter().enumerate() {
            // The line below belongs to this role only if it is not itself an anchor.
            let below = lines
                .get(i + 1)
                .filter(|_| i + 1 < end && !anchors.get(k + 1).is_some_and(|(j, _)| j == &(i + 1)))
                .filter(|l| !starts_with_bullet(l))
                .map(String::as_str);
            roles.push(role_for(lines, *i, range, below));
        }
    }
    roles
}
