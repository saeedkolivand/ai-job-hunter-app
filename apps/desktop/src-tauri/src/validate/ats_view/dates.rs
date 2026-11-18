//! Localized date-range parsing for the ATS view: `03/2021`, `März 2021`,
//! `2021–heute`, `Mar 2021 – Present`, `seit 03/2021`.
//!
//! Deliberately strict on what counts as PARSED (a four-digit year, a month
//! that is a real month) and loose on what counts as a date-SHAPED range
//! (`Jan '21 – Mar '22`, `03/21 - 05/22`): a range that is shaped like a date
//! but does not parse is exactly what a rigid parser mis-reads, so it is
//! reported as unparseable rather than ignored.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::PRESENT_MARKERS;

/// Present-tense words beyond [`PRESENT_MARKERS`], for the locales whose résumés the
/// generator writes with month names or a word for "now" (es, fr, it, pt).
const PRESENT_EXTRA: &[&str] = &[
    "aujourd'hui",
    "présent",
    "actualidad",
    "actualmente",
    "actuellement",
    "presente",
    "attualmente",
    "oggi",
    "hoy",
    "hoje",
    "atualmente",
    "derzeit",
    "today",
];

fn present_words() -> Vec<&'static str> {
    PRESENT_MARKERS
        .iter()
        .chain(PRESENT_EXTRA)
        .copied()
        .collect()
}

/// Month names and abbreviations for the locales the résumé generator writes dates in
/// (`pipeline::resume::prompt_blocks` `date_example`): en, de, es, fr, it, pt, nl.
/// Spellings shared between languages always agree on the number. The regex
/// alternation is built longest-first (see [`month_alt`]).
const MONTHS: &[(&str, u8)] = &[
    // es
    ("enero", 1),
    ("febrero", 2),
    ("marzo", 3),
    ("abril", 4),
    ("mayo", 5),
    ("junio", 6),
    ("julio", 7),
    ("agosto", 8),
    ("septiembre", 9),
    ("setiembre", 9),
    ("octubre", 10),
    ("noviembre", 11),
    ("diciembre", 12),
    ("ene", 1),
    ("abr", 4),
    ("ago", 8),
    ("dic", 12),
    // fr
    ("janvier", 1),
    ("février", 2),
    ("fevrier", 2),
    ("mars", 3),
    ("avril", 4),
    ("juin", 6),
    ("juillet", 7),
    ("août", 8),
    ("aout", 8),
    ("septembre", 9),
    ("décembre", 12),
    ("decembre", 12),
    ("janv", 1),
    ("févr", 2),
    ("fevr", 2),
    ("avr", 4),
    ("juil", 7),
    ("déc", 12),
    // it
    ("gennaio", 1),
    ("febbraio", 2),
    ("aprile", 4),
    ("maggio", 5),
    ("giugno", 6),
    ("luglio", 7),
    ("settembre", 9),
    ("dicembre", 12),
    ("gen", 1),
    ("mag", 5),
    ("giu", 6),
    ("lug", 7),
    ("set", 9),
    ("ott", 10),
    // pt
    ("janeiro", 1),
    ("fevereiro", 2),
    ("março", 3),
    ("maio", 5),
    ("junho", 6),
    ("julho", 7),
    ("setembro", 9),
    ("outubro", 10),
    ("dezembro", 12),
    ("fev", 2),
    ("out", 10),
    // nl
    ("januari", 1),
    ("februari", 2),
    ("maart", 3),
    ("mei", 5),
    ("augustus", 8),
    ("mrt", 3),
    // en + de (the original two)
    ("september", 9),
    ("dezember", 12),
    ("november", 11),
    ("december", 12),
    ("february", 2),
    ("oktober", 10),
    ("october", 10),
    ("january", 1),
    ("februar", 2),
    ("august", 8),
    ("januar", 1),
    ("march", 3),
    ("april", 4),
    ("juni", 6),
    ("june", 6),
    ("juli", 7),
    ("july", 7),
    ("märz", 3),
    ("maerz", 3),
    ("sept", 9),
    ("sep", 9),
    ("okt", 10),
    ("oct", 10),
    ("nov", 11),
    ("dez", 12),
    ("dec", 12),
    ("jan", 1),
    ("feb", 2),
    ("mär", 3),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("mai", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
];

const YEAR: &str = r"(?:19|20)\d{2}";
const SEP: &str = r"(?:[-–—]|\bto\b|\bbis\b|\buntil\b|\btill\b)";

/// A calendar point: a year and, when the source gave one, its month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Ym {
    pub(super) year: u16,
    pub(super) month: Option<u8>,
}

/// The end of a range: a real date or an open ("present") end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    Date(Ym),
    Present,
}

/// One date range found in a line. `from`/`to` are `None` when that side is
/// date-shaped but did not parse; `*_raw` is the text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Range2 {
    pub(super) span: Range<usize>,
    pub(super) from_raw: String,
    pub(super) to_raw: String,
    pub(super) from: Option<Ym>,
    pub(super) to: Option<End>,
}

/// Longest spelling first so the alternation never stops at an abbreviation.
fn month_alt() -> String {
    let mut names: Vec<&str> = MONTHS.iter().map(|(n, _)| *n).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    names.join("|")
}

/// `aujourd'hui`/`aujourd’hui` → one spelling, lowercased, for lookups.
fn norm_word(raw: &str) -> String {
    raw.trim().replace('’', "'").to_lowercase()
}

fn month_of(word: &str) -> Option<u8> {
    let w = norm_word(word.trim_end_matches('.'));
    MONTHS.iter().find(|(n, _)| *n == w).map(|(_, m)| *m)
}

/// Every strictly parseable point shape, as an unanchored pattern.
fn strict_point() -> String {
    format!(
        r"(?:(?:{m})\.?\s+{y}|(?:0?[1-9]|1[0-2])[./]{y}|{y}[-/.](?:0[1-9]|1[0-2])|{y})",
        m = month_alt(),
        y = YEAR
    )
}

/// Strict shapes plus the date-shaped forms that do NOT parse (two-digit
/// years, apostrophe years) — recognised only so they can be reported.
fn loose_point() -> String {
    format!(
        r"(?:{s}|(?:(?:{m})\.?\s*)?'\d{{2}}|(?:0?[1-9]|1[0-2])[./]\d{{2}})",
        s = strict_point(),
        m = month_alt()
    )
}

/// The present words as a regex alternation; the apostrophe also matches `’`.
fn present_alt() -> String {
    present_words()
        .iter()
        .map(|w| regex::escape(w).replace('\'', "['’]"))
        .collect::<Vec<_>>()
        .join("|")
}

static RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let present = present_alt();
    let point = loose_point();
    Regex::new(&format!(
        r"(?i)\b({point})\s*{SEP}\s*({point}|\d{{2}}\b|(?:{present})\b)"
    ))
    .unwrap()
});

/// `seit 03/2021`, `since 2021`: an open-ended range with only a start.
static OPEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:since|seit|from|ab|desde|depuis|dal|dallo|sinds)\s+({})",
        strict_point()
    ))
    .unwrap()
});

static POINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^(?:(?P<mon>{m})\.?\s+(?P<y1>{y})|(?P<n>\d{{1,2}})[./](?P<y2>{y})|(?P<y3>{y})[-/.](?P<n3>\d{{2}})|(?P<y4>{y}))$",
        m = month_alt(),
        y = YEAR
    ))
    .unwrap()
});

/// Parse one date point; `None` when it is not a real month/year.
pub(super) fn parse_point(raw: &str) -> Option<Ym> {
    let c = POINT_RE.captures(raw.trim())?;
    let year = |k: &str| c.name(k).and_then(|m| m.as_str().parse::<u16>().ok());
    let month = |m: u8| (1..=12).contains(&m).then_some(m);
    if let (Some(mon), Some(year)) = (c.name("mon"), year("y1")) {
        return Some(Ym {
            year,
            month: month_of(mon.as_str()),
        });
    }
    if let (Some(n), Some(year)) = (c.name("n"), year("y2")) {
        let m = month(n.as_str().parse().ok()?)?;
        return Some(Ym {
            year,
            month: Some(m),
        });
    }
    if let (Some(n), Some(year)) = (c.name("n3"), year("y3")) {
        let m = month(n.as_str().parse().ok()?)?;
        return Some(Ym {
            year,
            month: Some(m),
        });
    }
    year("y4").map(|year| Ym { year, month: None })
}

fn parse_end(raw: &str) -> Option<End> {
    let lower = norm_word(raw);
    if present_words().contains(&lower.as_str()) {
        return Some(End::Present);
    }
    parse_point(raw).map(End::Date)
}

/// The first date range in `line` (`A – B`, `A to B`, `A bis heute`, or the
/// open-ended `seit A`), parsed where possible.
pub(super) fn find_range(line: &str) -> Option<Range2> {
    if let Some(c) = RANGE_RE.captures(line) {
        let whole = c.get(0)?;
        let (a, b) = (c.get(1)?.as_str(), c.get(2)?.as_str());
        return Some(Range2 {
            span: whole.range(),
            from_raw: a.to_string(),
            to_raw: b.to_string(),
            from: parse_point(a),
            to: parse_end(b),
        });
    }
    let c = OPEN_RE.captures(line)?;
    let a = c.get(1)?.as_str();
    Some(Range2 {
        span: c.get(0)?.range(),
        from_raw: a.to_string(),
        to_raw: String::new(),
        from: parse_point(a),
        to: Some(End::Present),
    })
}

/// Machine value: `YYYY-MM`, `YYYY`, or `present`.
pub(super) fn point_value(p: Ym) -> String {
    match p.month {
        Some(m) => format!("{}-{m:02}", p.year),
        None => p.year.to_string(),
    }
}

pub(super) fn end_value(e: End) -> String {
    match e {
        End::Date(p) => point_value(p),
        End::Present => "present".to_string(),
    }
}
