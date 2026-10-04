//! `factual.unsourced_metric` — the figures a document claims against the
//! figures its source states.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::validate::content::{
    contains_phrase, has_real_contact_match, issue, split_sections, word_count, ContentIssue,
    DocKind, FACTUAL_UNSOURCED_METRIC,
};

/// A digit-bearing claim of impact. Only these three shapes are checked; a bare
/// one- or two-digit number ("3 engineers") is far too common to police.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetricKind {
    /// `40%`
    Percent,
    /// `3x`, `2.5×`
    Multiplier,
    /// `1,200` / `4500` — three digits or more, never a 1900–2099 year — and
    /// the EXPANDED value of a magnitude-suffixed figure (`10k` → `10000`, see
    /// [`SUFFIXED_NUMBER_RE`]), which is the same claim written shorter.
    LargeInteger,
}

/// One extracted metric: its kind, its number normalized to a comparable form,
/// and the span exactly as written (for the issue's `evidence`).
#[derive(Debug, Clone, PartialEq)]
pub struct Metric {
    pub kind: MetricKind,
    pub number: String,
    pub raw: String,
}

static PERCENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d[\d.,]*\d|\d)[\s\u{00A0}\u{202F}]*%").unwrap());

/// A multiplier: `3x`, `2.5×`.
///
/// The two spellings need DIFFERENT trailing rules, which is why the unit is an
/// alternation rather than a `[x×]` class. `x` is a word character, so `\b`
/// after it is what keeps `3xtra` out. `×` is NOT — it is itself the
/// non-word character a boundary needs on one side — so a trailing `\b` there
/// requires a WORD character to follow, i.e. it matched `3×5` and a line ending
/// in `3×` while silently skipping every mid-sentence "grew throughput 3×
/// while …". Those multipliers were never extracted and therefore never
/// cross-checked against the source at all.
static MULTIPLIER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d[\d.,]*\d|\d)\s*(?:x\b|×)").unwrap());

/// A written integer, in any digit-grouping convention.
///
/// Leading `\b` only, deliberately. Requiring a trailing boundary too would
/// miss `480ms`; dropping the leading one would make `sha256` yield "256".
///
/// The first arm exists because a space (or a Swiss apostrophe) is a grouping
/// separator in half of Europe, and [`normalize_number`] has always promised
/// `1 200` → `1200`. Without it the figure split into "1" and "200", so a
/// document truthfully restating its own source's `1 200` as `1,200` was
/// reported as having fabricated the number. It is strict where the second arm
/// is loose — EXACTLY three digits per group, which is what stops "ran 5 12
/// hour shifts" from reading as `512` — because a space between digits is
/// ordinary prose, while a `.` or `,` between them is not. `normalize_number`
/// stays the arbiter of what the digits MEAN; this only decides how far one
/// number reaches.
static INTEGER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(",
        r"\d{1,3}(?:[ \u{00A0}\u{202F}'\u{2019}]\d{3})+(?:[.,]\d+)?\b",
        r"|\d[\d.,\u{202F}\u{00A0}]*\d",
        r"|\d",
        r")"
    ))
    .unwrap()
});

/// A heading-less document (a cover letter) has no section band to skip, so the
/// letterhead is skipped by shape instead: metrics are read only from the body,
/// which starts at the first line long enough to be a sentence. An address line
/// ("10115 Berlin") is short; a claim of impact never is.
pub const MIN_WORDS_IN_LETTER_BODY_LINE: usize = 8;

/// Normalize a written number to a comparable string: drop digit-grouping
/// separators (`1,200` / `1.200` / `1 200` / `1'200` → `1200`) and render a
/// decimal comma as a period (`3,5` → `3.5`).
///
/// A separator counts as GROUPING when exactly three digits follow it and more
/// digits follow those or the number ends there; otherwise it is a decimal
/// point. Locale-neutral by construction, which matters because the source and
/// the generated text can be written in different markets' conventions.
///
/// Only `.` and `,` can ever BE a decimal point, so the space forms and the
/// Swiss apostrophe simply vanish either way — they are listed here anyway so
/// the separator set this function recognises is stated in one place, next to
/// the rule that arbitrates it. [`INTEGER_RE`] is what decides how far a
/// written number reaches; this decides what its digits mean, and the two must
/// agree on the same separators or a figure the regex captures whole
/// normalizes as if it were two.
pub fn normalize_number(raw: &str) -> String {
    let digits_and_seps: Vec<char> = raw
        .chars()
        .filter(|c| {
            c.is_ascii_digit()
                || matches!(
                    c,
                    '.' | ',' | '\u{202F}' | '\u{00A0}' | ' ' | '\'' | '\u{2019}'
                )
        })
        .collect();
    let mut out = String::with_capacity(digits_and_seps.len());
    let mut i = 0;
    while i < digits_and_seps.len() {
        let c = digits_and_seps[i];
        if c.is_ascii_digit() {
            out.push(c);
            i += 1;
            continue;
        }
        let following_digits = digits_and_seps[i + 1..]
            .iter()
            .take_while(|d| d.is_ascii_digit())
            .count();
        // Exactly three digits after the separator, and a digit before it →
        // thousands grouping. Anything else (one or two digits, or four+) is a
        // decimal separator.
        let grouping =
            following_digits == 3 && out.chars().next_back().is_some_and(|p| p.is_ascii_digit());
        if !grouping && matches!(c, '.' | ',') {
            out.push('.');
        }
        i += 1;
    }
    out.trim_end_matches('.').to_string()
}

/// Which document a band walk is being computed for. The two sides of the
/// metric comparison ask different questions of the same text, and the ONE
/// place they may legitimately differ is the pre-heading band — see
/// [`metric_lines`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum MetricSide {
    /// The generated document: its numbers are CLAIMS the user is answerable
    /// for.
    Claims,
    /// The source document: its numbers are what the candidate already stated.
    Source,
}

/// Digits a number-only line must carry before it reads as a phone number
/// rather than as a figure. Seven is the floor
/// `super::HEADER_PHONE_RE`'s marker-less arm already applies, kept the same so
/// the two "is this a phone?" answers cannot disagree about length.
pub const MIN_BARE_PHONE_LINE_DIGITS: usize = 7;

/// True when a line is NOTHING but a phone-shaped token: enough digits to be a
/// subscriber number, and not one other character besides the separators phone
/// numbers are written with.
///
/// Deliberately looser about SHAPE than [`super::looks_like_header_phone`],
/// because being alone on a line is itself the signal. That helper answers "is
/// this line contact details?" and has to stay strict, since ordinary numeric
/// prose ("150 - 200 EUR per hour") would otherwise exempt itself from the whole
/// metric pass — its documented cost is a bare US number with no `+`/parens
/// ("555-123-4567", longest digit run four), which is exactly what a letter
/// signs off with. This asks a much narrower question: a line carrying no words
/// at all makes no claim of impact, whatever its digits are, so relaxing the
/// shape here cannot exempt any prose. `INTEGER_RE` used to read that sign-off
/// as THREE fabricated figures ("555", "123", "4567").
///
/// *Accepted cost, stated:* a line consisting of nothing but bare numbers
/// totalling seven digits or more is not read as a claim either. A metric needs
/// a sentence around it to mean anything, and this file's rule is that a missed
/// check beats a wrong accusation.
///
/// **Not symmetric — see [`metric_lines`]' second invariant.** "This makes a
/// poor claim" is a statement about the CLAIMS side. Applied to the source it
/// says "this is not a statement either", which is false: the same shape is a
/// candidate's own figure on its own line, and striking it from the sourced set
/// accuses them of inventing it. On the source side this rule therefore runs
/// inside the contact band only.
fn is_bare_phone_line(text: &str) -> bool {
    let line = text.trim();
    line.chars().filter(char::is_ascii_digit).count() >= MIN_BARE_PHONE_LINE_DIGITS
        && line.chars().all(|c| {
            c.is_ascii_digit()
                || matches!(
                    c,
                    '+' | '(' | ')' | '-' | '–' | '.' | '/' | ' ' | '\u{00A0}'
                )
        })
}

/// The lines of `text` whose numbers count — CLAIMS on the generated side, what
/// the source already stated on the truth side.
///
/// One walk for both, because the sides disagreeing is what makes a hole: the
/// truth side used to scan the raw text, so the candidate's own phone digits
/// "sourced" a fabricated figure that merely reused them.
///
/// What the band is depends on whether the document HAS sections:
///
/// * **A résumé** (headings present) — the band is section 0, name + contact.
///   On the CLAIMS side it is skipped by POSITION and nothing else is exempt.
///   The per-line exemption that used to sit here ("the parser called it
///   `Contact` and it carries a real address or phone") reached every section,
///   and a wrapped body paragraph is contact-shaped whenever it carries a
///   European-grouped figure ("90 000 - 110 000" satisfies `PHONE_RE`), so a
///   fabricated number in the middle of a document was exempt from the whole
///   check.
/// * **A cover letter** (no headings) — there is no positional band, so the
///   letterhead is skipped by SHAPE: the body starts at the first line long
///   enough to be a sentence ([`MIN_WORDS_IN_LETTER_BODY_LINE`]), and a real
///   contact line is skipped wherever it sits, because a letter carries contact
///   details in its letterhead AND its sign-off. That shape test is
///   [`has_real_contact_match`] — the same one `ats::is_contact_cluster` applies
///   inline, minus the `LineKind` coupling this fix removed.
///
/// ## Why the SOURCE side reads section 0 by shape instead of position
///
/// A résumé that opens with a profile paragraph BEFORE its first heading is an
/// ordinary layout, and skipping section 0 wholesale erased every figure in that
/// paragraph from the sourced set — so the same figure, restated under the
/// generated document's `SUMMARY` heading, came back as a fabrication Critical.
/// On the truth side the band is therefore filtered by
/// [`has_real_contact_match`] (plus [`is_bare_phone_line`]) rather than dropped.
///
/// ## The two invariants, precisely
///
/// They point in opposite directions, which is exactly why the side is a
/// parameter and not a comment:
///
/// 1. **`Source` ⊇ `Claims`.** The source side never reads fewer lines than the
///    claims side, so the asymmetry can only ever silence a finding, never raise
///    one.
/// 2. **The source side may drop a line only because it is contact DETAILS** —
///    never merely because its shape makes a poor claim. Every line dropped from
///    the source is a number that can no longer vouch for its own restatement,
///    i.e. an accusation channel, so (1) alone is not enough: a rule applied
///    identically to both sides satisfies (1) and still opens one.
///
/// [`is_bare_phone_line`] used to sit outside the side test and so broke (2):
/// a figure alone on a line — the ordinary output of a table-extracted PDF —
/// was struck from the sourced set everywhere, and restating it read as
/// fabrication. It now applies on the claims side in every band, and on the
/// source side only inside the contact band, where the shape really does mean a
/// phone number. Under (1) that is a strict widening of the source side.
///
/// So the invariant the earlier rounds were reaching for holds in this exact
/// form: **contact details never source a body metric**, where "contact
/// details" means the contact BAND filtered by shape, not any line that happens
/// to look numeric.
///
/// ## (2) is a statement about the two SHAPE TESTS agreeing, not just the sides
///
/// The band keeps TWO tests — [`has_real_contact_match`] and
/// [`is_bare_phone_line`] — and they must answer the same way about the same
/// number however it is written, or a line the source drops is one the claims
/// side keeps and the invariant fails across the two documents rather than
/// inside one. That is exactly what a too-broad guard in
/// [`super::looks_like_header_phone`] did: a contact-band line reading
/// "+49 30 2019 1234" was dropped from the truth set as a bare phone line, while
/// the letter restating the same number with words around it kept it (the shape
/// test refused it for carrying a year), and the candidate's own phone came back
/// as a fabricated metric. The repair is in the shape test, where the statement
/// belongs — a DATE SPAN is not a phone number, a year-shaped run inside one is.
///
/// *Residual, stated rather than hidden:* a contact-shaped line the shape test
/// misses (a long prose letterhead, an address line with no phone or email) is
/// read as source text, so its digits can vouch for a claim. That is a missed
/// check, which is this family's chosen direction of error.
///
/// ## `doc_kind` is the kind of THIS TEXT
///
/// "Does the document have sections?" is `sections.len() > 1`, and
/// [`super::split_sections`] can MANUFACTURE a section by promoting an
/// unrecognised line to a heading. In a letter — which never has a parser
/// heading — a single short label line was enough to flip `has_headings` and put
/// the whole opening of the letter behind the résumé path's positional band
/// skip. Promotion is therefore a résumé repair only, and the kind is passed per
/// text rather than per report: the truth side of a LETTER's comparison is the
/// candidate's own résumé and still needs it.
fn metric_lines(text: &str, side: MetricSide, doc_kind: DocKind) -> Vec<String> {
    let sections = split_sections(text, doc_kind);
    let has_headings = sections.len() > 1;
    let mut out = Vec::new();
    for (idx, section) in sections.iter().enumerate() {
        let contact_band = has_headings && idx == 0;
        if contact_band && side == MetricSide::Claims {
            continue;
        }
        let mut body_started = has_headings;
        for line in &section.lines {
            if !body_started {
                body_started = word_count(&line.text) >= MIN_WORDS_IN_LETTER_BODY_LINE;
                if !body_started {
                    continue;
                }
            }
            // Every band whose contact lines are not skipped by POSITION is
            // skipped by SHAPE instead: the letter (no headings at all) and the
            // source résumé's pre-heading band.
            if (contact_band || !has_headings) && has_real_contact_match(&line.text) {
                continue;
            }
            // …and a line that is nothing but a phone number is not a CLAIM in
            // any band, including the body a sign-off sits in. On the SOURCE
            // side the same line is a statement, and a statement may only ever
            // silence — so it is dropped there ONLY inside the contact band,
            // where a phone number is what it is. Everywhere else in the source
            // this shape is the candidate's own figure alone on a line, the
            // ordinary output of a table-extracted PDF, and dropping it turned
            // a restatement of that figure into a fabrication Critical.
            if (side == MetricSide::Claims || contact_band) && is_bare_phone_line(&line.text) {
                continue;
            }
            out.push(line.text.clone());
        }
    }
    out
}

/// Extract every impact metric from `text`, skipping the contact band (see
/// [`metric_lines`]).
///
/// Also skipped, inside the lines that are scanned:
/// * 1900–2099 four-digit runs — those are years, checked by
///   [`unsupported_date_issues`] instead;
/// * numbers under three digits with no `%`/`x` unit.
pub fn metrics_in(text: &str, doc_kind: DocKind) -> Vec<Metric> {
    metrics_in_lines(&metric_lines(text, MetricSide::Claims, doc_kind))
}

fn metrics_in_lines(lines: &[String]) -> Vec<Metric> {
    let mut out = Vec::new();
    for line in lines {
        collect_metrics(line, &mut out);
    }
    out
}

/// Extract the metrics one line states.
///
/// ## Both sides speak one number language
///
/// [`SUFFIXED_NUMBER_RE`] used to run on the SOURCE side only, as a leniency:
/// a source writing "10k" covers a generated "10,000". Read the other way round
/// that leniency was a HOLE. `INTEGER_RE` sees only the mantissa of `10k`, and
/// the mantissa is discarded below three digits ("35k" → "35") or on a decimal
/// point ("3.5m" → "3.5"), so a *fabricated* suffixed figure was not tolerated,
/// it was structurally invisible: `unsourced_metric` never saw a claim to check.
///
/// The mantissa is also why the suffixed pass has to SUPPRESS the integer
/// capture inside its span rather than sit beside it. "250k" left "250" behind
/// as a claim of its own, which a source writing "250,000" never states — a
/// false Critical on a truthful restatement, and post-fix it would have been a
/// second Critical about the same span on a fabricated one, which the
/// deduplication in [`unsourced_metric_issues`] exists to prevent.
///
/// The SOURCE side loses nothing to that suppression: `sourced` also chains
/// [`all_numbers`], which runs `INTEGER_RE` unfiltered over the same lines, so
/// the source keeps both readings of its own figure. `Source ⊇ Claims` holds.
fn collect_metrics(line: &str, out: &mut Vec<Metric>) {
    // Magnitude-suffixed figures first, so the integer pass below can skip what
    // they already claimed. No year exclusion here, unlike the integer arm:
    // that rule exists because a résumé is full of four-digit years, and `2k`
    // is not how anyone writes one.
    let mut suffixed_spans: Vec<(usize, usize)> = Vec::new();
    for caps in SUFFIXED_NUMBER_RE.captures_iter(line) {
        let Some(span) = caps.get(0) else { continue };
        suffixed_spans.push((span.start(), span.end()));
        if let Some(number) = expand_suffixed(&caps) {
            out.push(Metric {
                kind: MetricKind::LargeInteger,
                number,
                raw: span.as_str().trim().to_string(),
            });
        }
    }
    for caps in PERCENT_RE.captures_iter(line) {
        out.push(Metric {
            kind: MetricKind::Percent,
            number: normalize_number(&caps[1]),
            raw: caps[0].trim().to_string(),
        });
    }
    for caps in MULTIPLIER_RE.captures_iter(line) {
        out.push(Metric {
            kind: MetricKind::Multiplier,
            number: normalize_number(&caps[1]),
            raw: caps[0].trim().to_string(),
        });
    }
    for caps in INTEGER_RE.captures_iter(line) {
        let Some(span) = caps.get(1) else { continue };
        // The mantissa of a suffixed figure is not a second claim — see above.
        if suffixed_spans
            .iter()
            .any(|(start, end)| span.start() >= *start && span.end() <= *end)
        {
            continue;
        }
        let raw = span.as_str().to_string();
        let normalized = normalize_number(&raw);
        // Three significant digits or more, and never a year.
        let digits = normalized.chars().filter(char::is_ascii_digit).count();
        let is_year = normalized
            .parse::<u32>()
            .is_ok_and(|n| (1900..=2099).contains(&n));
        if digits >= 3 && !is_year && !normalized.contains('.') {
            out.push(Metric {
                kind: MetricKind::LargeInteger,
                number: normalized,
                raw,
            });
        }
    }
}

/// `factual.unsourced_metric` — a number the generated document claims that the
/// truth text never states.
///
/// ## What counts as "the source already said this"
///
/// The check is deliberately lenient about HOW the source wrote the number,
/// because a tailored bullet restates facts rather than copying them, and a
/// false "you fabricated this" is the finding that makes a user stop reading the
/// panel. A number is treated as sourced when the source states it
///
/// * anywhere at all, in any unit — "cut latency by 40 percent" covers a
///   generated "40%" (the position and the unit are not compared, only the
///   normalized digits);
/// * in any digit-grouping or decimal convention — `1,200` / `1.200` / `1 200`
///   all normalize to `1200` (see [`normalize_number`]);
/// * with a magnitude suffix — `10k` covers `10,000`, `3m` covers `3000000`
///   (see [`SUFFIXED_NUMBER_RE`]);
/// * as a word rather than a digit — "doubled throughput" covers a generated
///   `2x` (see [`WORD_NUMBERS`]).
///
/// What it does NOT do is verify that the number is attached to the same claim.
/// That needs meaning, and guessing at meaning is how a deterministic check
/// starts accusing people.
pub(super) fn unsourced_metric_issues(
    generated: &str,
    truth: &str,
    doc_kind: DocKind,
) -> Vec<ContentIssue> {
    // The truth's band-skipped lines, resolved ONCE: both number passes below
    // read them, and both must skip the same CONTACT band as the generated side
    // does, or the source's contact digits become evidence for a claim (see
    // [`metric_lines`], which also documents the one place the two sides
    // deliberately differ).
    //
    // The truth is always a RÉSUMÉ (the candidate's own, plus the job ad on the
    // letter path), whatever `doc_kind` the CLAIMS side is — see `metric_lines`'
    // `doc_kind` section.
    let truth_lines = metric_lines(truth, MetricSide::Source, DocKind::Resume);
    let sourced: HashSet<String> = metrics_in_lines(&truth_lines)
        .into_iter()
        .map(|m| m.number)
        // Bare numbers, magnitude suffixes and word-numbers in the truth text
        // all count — see the doc comment.
        .chain(all_numbers(&truth_lines))
        .chain(suffixed_numbers(truth))
        .chain(word_numbers(truth))
        .collect();
    // Deduplicated on the NORMALIZED number, the same key the sourcing decision
    // above is taken on. The raw span is not that key: `PERCENT_RE` and
    // `INTEGER_RE` both match a three-digit percentage, yielding "150%" and
    // "150" for one fabricated figure — two Criticals about the same span, on
    // the family whose whole design rule is that a wrong Critical is worse than
    // a missed one. The first spelling encountered wins, so the evidence still
    // quotes the unit the document actually wrote ("150%", not "150").
    let mut seen = HashSet::new();
    metrics_in(generated, doc_kind)
        .into_iter()
        .filter(|m| !sourced.contains(&m.number))
        .filter(|m| seen.insert(m.number.clone()))
        .map(|m| {
            issue(
                FACTUAL_UNSOURCED_METRIC,
                None,
                format!(
                    "\"{}\" does not appear in your source résumé. Replace it with a figure \
                     your own document supports, or remove the claim.",
                    m.raw
                ),
                Some(m.raw),
            )
        })
        .collect()
}

/// Every number on `lines`, normalized — the lenient half of the metric check.
///
/// Takes the already-band-skipped lines rather than the raw document on
/// purpose. Scanning the whole source here let the candidate's own contact
/// details vouch for a fabricated figure: a header reading "+49 30 1234567"
/// put `1234567` into the sourced set, so a bullet claiming that many
/// settlements passed silently, and a postal code did the same for any
/// five-digit claim. The two sides of the comparison must skip the same band or
/// the asymmetry IS the hole.
fn all_numbers(lines: &[String]) -> HashSet<String> {
    lines
        .iter()
        .flat_map(|line| INTEGER_RE.captures_iter(line))
        .map(|c| normalize_number(&c[1]))
        .filter(|n| !n.is_empty())
        .collect()
}

/// A number written with a magnitude suffix: `10k`, `3.5k`, `2m`, `1bn`.
///
/// The trailing `\b` is what keeps `480ms` and `90ms` out: `m` followed by `s`
/// is not a word boundary, so a millisecond figure never reads as millions.
static SUFFIXED_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d[\d.,\u{202F}\u{00A0}]*\d|\d)\s*(bn|k|m)\b").unwrap());

/// The expanded value of one [`SUFFIXED_NUMBER_RE`] match (`10k` → `10000`),
/// or `None` when the scaled mantissa is not a finite whole number.
///
/// **The single expansion, deliberately shared.** The source side reads it to
/// decide a figure was already stated and the claims side reads it to decide
/// what the document asserts ([`collect_metrics`]); the two must expand
/// identically or the comparison compares different numbers.
fn expand_suffixed(caps: &regex::Captures<'_>) -> Option<String> {
    let scale: f64 = match caps[2].to_ascii_lowercase().as_str() {
        "k" => 1_000.0,
        "m" => 1_000_000.0,
        _ => 1_000_000_000.0,
    };
    let value: f64 = normalize_number(&caps[1]).parse().ok()?;
    let expanded = value * scale;
    (expanded.fract() == 0.0 && expanded.is_finite()).then(|| format!("{expanded:.0}"))
}

/// Magnitude-suffixed numbers in `text`, EXPANDED (`10k` → `10000`).
///
/// The SOURCE side's half of the pass: a source that writes "10k requests" and
/// a generated bullet that writes "10,000 requests" state the same fact, and
/// the whole point is that a restatement is not a fabrication. The CLAIMS side
/// runs the same regex through [`collect_metrics`], where the expanded value
/// becomes a checkable claim rather than a sourced one.
fn suffixed_numbers(text: &str) -> HashSet<String> {
    SUFFIXED_NUMBER_RE
        .captures_iter(text)
        .filter_map(|caps| expand_suffixed(&caps))
        .collect()
}

/// Words that state a multiplier without a digit. A source writing "doubled
/// throughput" and a generated bullet writing "2x throughput" are the same
/// claim; without this pair the restatement reads as an invented figure.
///
/// Deliberately tiny and one-directional (word → digits): these are the only
/// two multipliers a résumé states in words often enough to matter, and every
/// entry added here weakens a Critical, so the bar is "seen in real output".
///
/// **Source-side only, unlike the magnitude suffixes** — the one place the two
/// sides may legitimately differ, and for a reason that is about the numbers
/// rather than about symmetry. A word-number expands to a SINGLE DIGIT, and a
/// one- or two-digit figure is not a claim this file polices at all (see
/// [`MetricKind`]): reading "double" as the claim `2` would make "double-entry
/// bookkeeping" a fabricated metric on any source that never writes a 2. A
/// magnitude suffix has the opposite shape — it only ever expands UPWARD, into
/// exactly the range the check is for.
const WORD_NUMBERS: &[(&str, &str)] = &[
    ("doubled", "2"),
    ("double", "2"),
    ("verdoppelt", "2"),
    ("tripled", "3"),
    ("triple", "3"),
    ("verdreifacht", "3"),
];

/// The digit forms of every word-number `text` states, word-bounded.
fn word_numbers(text: &str) -> HashSet<String> {
    let lower = text.to_lowercase();
    WORD_NUMBERS
        .iter()
        .filter(|(word, _)| contains_phrase(&lower, word))
        .map(|(_, digits)| (*digits).to_string())
        .collect()
}
