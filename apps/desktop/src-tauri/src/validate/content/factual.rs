//! Factual grounding — the Critical family.
//!
//! Every check here compares the generated text against the candidate's own
//! source document(s). Nothing in this file consults a model, and nothing
//! guesses: if a comparison cannot be made reliably it is skipped, because a
//! false "you fabricated this" is worse than a missed one.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::{
    identity_tokens, split_entry, trailing_date_column, years_in, SectionKind, LEGAL_FORMS,
};
use crate::documents::keywords::{keywords_normalized, SHORT_TECH_TERMS, SYNONYMS};
use crate::export::types::{LineKind, ParsedLine};

use super::{
    contains_phrase, has_real_contact_match, issue, Analysis, ContentIssue, DocKind, Section,
    FACTUAL_ALTERED_PROJECT_LINK, FACTUAL_DROPPED_ROLE, FACTUAL_UNSOURCED_METRIC,
    FACTUAL_UNSOURCED_TERM, FACTUAL_UNSUPPORTED_DATE,
};

/// Minimum characters of a company token before it counts as distinctive
/// enough to decide a role went missing. "AG", "Inc" or "The" appearing
/// nowhere in the output proves nothing.
pub const MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS: usize = 4;

/// Minimum characters of a company token that counts as EVIDENCE the entry
/// survived. Two, not four — see [`survival_tokens`] for why the two bars
/// differ.
pub const MIN_SURVIVAL_COMPANY_TOKEN_CHARS: usize = 2;

/// Hard cap on how many employment entries the entry-vs-document scans consider
/// — [`dropped_role_issues`] here and `consistency::title_drift_issues`, which
/// imports this constant rather than picking its own.
///
/// Both are O(entries × document): the first substring-searches the whole
/// generated text once per SOURCE entry, the second compares every generated
/// entry against every source entry. Neither input is trusted or small (the save
/// path admits ~200KB of each), so an entry count is bounded before the
/// expensive thing, exactly as `duplicates::MAX_DUP_BULLETS` bounds the O(n²)
/// near-duplicate scan for the same reason. A real résumé has a dozen roles; the
/// cap only ever bites on already-broken input, and it caps the SCAN, never
/// `count_roles` — the `rolesSource`/`rolesOutput` metric stays honest.
pub const MAX_SCANNED_ENTRIES: usize = 200;

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
    let sections = super::split_sections(text, doc_kind);
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
                body_started = super::word_count(&line.text) >= MIN_WORDS_IN_LETTER_BODY_LINE;
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
fn unsourced_metric_issues(generated: &str, truth: &str, doc_kind: DocKind) -> Vec<ContentIssue> {
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

/// Company tokens distinctive enough to decide whether an entry is CHECKABLE at
/// all.
///
/// Lowercased alphanumeric tokens of [`MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS`]+
/// characters, minus legal-form suffixes that carry no identity. An entry with
/// none of these is skipped rather than guessed at.
///
/// Geography is deliberately NOT excluded here — it is what makes "IBM
/// Deutschland GmbH" a checkable entry at all — which is why this cannot simply
/// be [`identity_tokens`].
fn distinctive_tokens(company: &str) -> Vec<String> {
    company
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|t| t.chars().count() >= MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS)
        .filter(|t| !LEGAL_FORMS.contains(&t.as_str()))
        .collect()
}

/// Company tokens that count as EVIDENCE the entry survived — down to
/// [`MIN_SURVIVAL_COMPANY_TOKEN_CHARS`] characters, because that is how short a
/// real employer's name gets.
///
/// The asymmetry with [`distinctive_tokens`] is the whole point. Deciding an
/// entry is checkable needs a distinctive 4+ character token; deciding it
/// survived must accept anything the candidate might reasonably have written,
/// or "IBM Deutschland GmbH" in the source and "IBM" in the output reads as a
/// dropped role — the source's only 4+ token is "deutschland", and the output
/// never says it.
///
/// Legal forms and geography are excluded here for the opposite reason they are
/// kept above: they must never be the sole evidence of survival, since every
/// German résumé mentions "GmbH" and "Berlin" somewhere. The lists live in
/// `documents::evidence` so `consistency::titled_entries` matches employers on
/// the same identity tokens this decides survival on.
fn survival_tokens(company: &str) -> Vec<String> {
    identity_tokens(company, MIN_SURVIVAL_COMPANY_TOKEN_CHARS)
}

/// The generated document's own EXPERIENCE section(s), lowercased and joined —
/// the ONLY place an employment entry legitimately lives.
///
/// [`company_survives`] used to search the WHOLE document. That let a Summary
/// sentence that merely NAMES a former employer ("…at Acme Payments and
/// Globex Logistics") count as evidence the entry survived, even after a
/// repair round deleted the entire Globex Logistics entry from EXPERIENCE —
/// the round-worse guard never saw a `factual.dropped_role`, and a job
/// silently vanished from the résumé. Scoping the search to the section an
/// entry actually lives in closes that hole while staying deliberately
/// generous WITHIN it: a legitimate rewrite may reword a company's
/// surrounding bullets, and every line of the section (not just its heading)
/// is still searched.
///
/// Falls back to the WHOLE document when the generated résumé has no
/// `SectionKind::Experience` section at all. "Absent" is not the same fact as
/// "deleted", and conflating them is a false accusation on a truthful
/// document: `classify_section` recognises `work experience` /
/// `berufserfahrung` / `employment` and a few more, but NOT `Work History` or
/// `Selected Roles`, which land in `Other`. Measured on a résumé carrying both
/// roles verbatim — heading `WORK EXPERIENCE` gives 0 issues, `WORK HISTORY`
/// gives 2 `factual.dropped_role` Criticals. Those are terminal:
/// `criticals_by_section` resolves them to `SectionKey::Experience`, which
/// `find` cannot locate, so no repair runs and the user is told two jobs
/// vanished from a document that still contains them.
///
/// The sibling `project_link_issues` already guards its absent section
/// explicitly; this mirrors it, preferring the pre-scoping behaviour (a
/// possible MISS) over a guaranteed false Critical — the scoping exists to
/// catch a rare regression and must not manufacture a common one.
///
/// **Existence, not emptiness, decides the fallback.** An earlier version
/// tested `scoped.is_empty()` — literally no text under the Experience
/// heading — which conflates two different situations: "no Experience section
/// exists" (fall back, correctly) and "an Experience section exists, but its
/// body was wiped" (also empty, but every entry under it WAS dropped). The
/// second case searched the whole document instead, found the employer's name
/// in an untouched Summary sentence, and read a wiped work history as fine.
/// The fallback is now gated on whether a `SectionKind::Experience` section is
/// present at all, so a present-but-wiped section stays scoped to its own
/// (empty) text and every entry correctly reads as dropped.
fn generated_experience_lower(sections: &[Section]) -> String {
    let joined = |only_experience: bool| {
        sections
            .iter()
            .filter(|s| !only_experience || s.kind == SectionKind::Experience)
            .flat_map(|s| s.lines.iter())
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase()
    };
    let has_experience = sections.iter().any(|s| s.kind == SectionKind::Experience);
    if !has_experience {
        return joined(false);
    }
    joined(true)
}

/// Whether `company` still appears in the generated text.
///
/// A long token matches as a SUBSTRING, because German compounds a company name
/// straight into the next word ("Sparkassen-Gruppe" evidences "Sparkasse"). A
/// short one needs word boundaries, because two or three letters occur inside
/// unrelated words all the time — "SAP" must not be evidenced by "sapphire".
fn company_survives(generated_lower: &str, company: &str) -> bool {
    survival_tokens(company).iter().any(|token| {
        if token.chars().count() >= MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS {
            generated_lower.contains(token.as_str())
        } else {
            contains_phrase(generated_lower, token)
        }
    })
}

/// Employment entries in a document, as `(company, dates)` pairs — split by
/// the SAME heuristic `documents::evidence` uses, so both surfaces agree on
/// which part of an entry line names the employer.
fn entries(sections: &[Section]) -> Vec<(String, String)> {
    sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .flat_map(|s| s.lines.iter())
        .filter(|l| matches!(l.kind, LineKind::JobEntry))
        .map(|l| {
            let (company, _title, dates) = split_entry(l);
            (company, dates)
        })
        .collect()
}

/// How many employment entries a document has — the `rolesSource`/`rolesOutput`
/// metric.
pub(super) fn count_roles(sections: &[Section]) -> usize {
    entries(sections).len()
}

/// `factual.dropped_role` — an employment entry the source résumé carries that
/// the generated document does not.
///
/// ## Heuristic
///
/// Two different token sets, on purpose, because "can I check this entry?" and
/// "did this entry survive?" are different questions:
///
/// * **CHECKABLE** needs a distinctive company token — 4+ characters, legal
///   forms removed ([`distinctive_tokens`]) — AND at least one survival token
///   to look for. An entry with neither is skipped rather than guessed at.
/// * **SURVIVED** accepts any company token of two characters or more
///   ([`company_survives`]). A shortened company name is normal tailoring: "IBM
///   Deutschland GmbH" written as "IBM", "SAP SE" as "SAP". Requiring the
///   *distinctive* token to survive turned every one of those into a Critical
///   claiming a role had vanished — the only 4+ token in "IBM Deutschland GmbH"
///   is "deutschland", which a tailored document has no reason to keep.
///
/// Legal forms and geography can never be the sole survival evidence: every
/// German résumé says "GmbH" and names a city somewhere, so accepting those
/// would spare an employer that really had been dropped.
///
/// The narrow reading is deliberate: shortening a role is a legitimate tailoring
/// decision, whereas a company vanishing from the document entirely is the loss
/// the candidate would never notice until an interviewer did.
fn dropped_role_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    // Scoped to the generated EXPERIENCE section(s), not the whole document —
    // see [`generated_experience_lower`].
    let generated_lower = generated_experience_lower(&ctx.generated_sections);
    entries(&ctx.source_sections)
        .into_iter()
        // Bound the scan before the expensive part: each surviving entry
        // substring-searches the generated EXPERIENCE text. See
        // [`MAX_SCANNED_ENTRIES`].
        .take(MAX_SCANNED_ENTRIES)
        .filter_map(|(company, dates)| {
            // Not checkable — never guessed at. BOTH halves are required: an
            // entry with no distinctive token cannot be identified, and one
            // with no SURVIVAL token has no evidence that could ever spare it,
            // so `company_survives` would answer false however faithful the
            // output is. "Deutschland GmbH", "Global Group" and a bare "Berlin"
            // are all in the second bucket, and each produced an unavoidable
            // Critical while the employer sat verbatim in the document.
            if distinctive_tokens(&company).is_empty() || survival_tokens(&company).is_empty() {
                return None;
            }
            if company_survives(&generated_lower, &company) {
                return None;
            }
            let years = years_in(&dates);
            let span = if years.is_empty() {
                dates.trim().to_string()
            } else {
                years
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join("–")
            };
            Some(issue(
                FACTUAL_DROPPED_ROLE,
                Some("Experience"),
                format!(
                    "Your source résumé lists \"{company}\" ({span}) but the generated document \
                     never mentions it. An unexplained gap is harder to defend than a short \
                     entry — add it back or shorten it instead."
                ),
                Some(company),
            ))
        })
        .collect()
}

/// `factual.unsupported_date` — a year in a date position that the source
/// résumé does not contain.
///
/// ## Heuristic
///
/// Only years found inside a date-shaped context are considered, and only
/// years the source never states. Even then it fires only when the year is
/// EARLIER than the latest year the source knows about: an invented earlier date
/// can never be a legitimate resolution of anything.
///
/// A LATER year is always let through. A source that says `2021 – Present` and
/// output that says `2021 – 2026` is the same fact with the open end resolved.
/// This check used to fire anyway whenever the source carried no recognisable
/// open-ended marker — which meant every source spelling its open end as
/// `seit 2021`, `since 2021` or a bare `2021 –` produced a Critical on truthful
/// output. Detecting those spellings is now
/// `documents::evidence::is_open_ended`'s job, and this check no longer needs to
/// ask: a later year is never reported either way.
///
/// **What counts as a date-shaped context.** A parsed `JobEntry` line, or a
/// line whose text ends in a [`trailing_date_column`] — the same structural
/// predicate `validate::content::labels_the_entry_below` uses, so the two
/// surfaces cannot drift about what "opens an entry" means. This used to be a
/// raw `PRESENT_MARKERS` word-boundary scan over the WHOLE line, which read
/// any ordinary bullet carrying one of those words (`current`, `ongoing`,
/// `now`, `actual`…) — anywhere in the sentence, regardless of a year's
/// position — as a date context: "Reduced actual costs by 20% in 2023" turned
/// its own truthful year into a false Critical. A present-tense word inside a
/// bullet no longer decides this; only actual date structure does.
///
/// Deterministic on purpose: the check never reads the system clock.
fn unsupported_date_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source_years: HashSet<u32> = years_in(ctx.input.source_resume).into_iter().collect();
    let Some(&source_max) = source_years.iter().max() else {
        return Vec::new(); // No dates to compare against — stay quiet.
    };

    let mut seen = HashSet::new();
    let mut issues = Vec::new();
    for section in &ctx.generated_sections {
        for line in &section.lines {
            let date_context = matches!(line.kind, LineKind::JobEntry)
                || trailing_date_column(&line.text).is_some();
            if !date_context {
                continue;
            }
            let dates = line.right_text.as_deref().unwrap_or(&line.text);
            for year in years_in(dates) {
                if source_years.contains(&year) || !seen.insert(year) {
                    continue;
                }
                if year < source_max {
                    issues.push(issue(
                        FACTUAL_UNSUPPORTED_DATE,
                        section.heading.as_deref(),
                        format!(
                            "The date {year} is not in your source résumé. Correct it to a date \
                             your own document supports."
                        ),
                        Some(year.to_string()),
                    ));
                }
            }
        }
    }
    issues
}

/// Hosts whose bare (scheme-less) form is unambiguously a link rather than a
/// library name. Everything else needs a scheme, a `www.`, or a `/path`.
const CODE_HOSTS: &str = "github|gitlab|bitbucket|codeberg|sourcehut|sourceforge|npmjs|crates|pypi|huggingface|gitea|launchpad";

/// A URL in résumé prose.
///
/// Four arms, and the fourth one is the whole reason this regex is not simply
/// "host dot TLD": a scheme-less bare host reads a stack line's library names as
/// links. `Socket.IO` is `.io`, `Bun.sh` is `.sh`, `Nuxt.dev` is `.dev` — and a
/// stack line listing three of them produced three Critical
/// `factual.altered_project_link` findings on a truthful résumé. So a bare host
/// must either be a known code host or carry a `/path`.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Assembled from parts, never a `\`-continued raw string: a raw string does
    // not process escapes, so a trailing backslash would leave a literal
    // `\<spaces>` inside the pattern and silently kill the arm before it.
    let pattern = [
        r"(?i)(?:https?://[^\s)\]<>]+",
        r"|www\.[^\s)\]<>]+",
        &format!(r"|(?:{CODE_HOSTS})\.(?:com|org|io|dev|net|rs)(?:/[^\s)\]<>]*)?"),
        r"|[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:com|org|net|io|dev|app|de|co|ai|sh|me)/[^\s)\]<>]*)",
    ]
    .concat();
    Regex::new(&pattern).unwrap()
});

/// Every URL in `text`, VERBATIM, with trailing sentence punctuation trimmed.
/// Markdown link targets are captured by the same pass — the regex matches the
/// href inside `[anchor](href)` as well as a bare URL, and `)` is excluded from
/// every arm's character class so the href stops at the closing paren.
pub fn urls_in(text: &str) -> Vec<String> {
    URL_RE
        .find_iter(text)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';', ':'])
                .to_string()
        })
        .collect()
}

/// The comparison key for a link: scheme dropped, one leading `www.` dropped,
/// host lowercased, one trailing `/` removed. The PATH keeps its case —
/// `/JaneDoe/Ledger` and `/janedoe/ledger` are different resources on most
/// hosts, and treating them as one would hide a genuinely altered link.
///
/// Compared on the key, REPORTED verbatim. `https://github.com/janedoe/ledger`
/// and `github.com/janedoe/ledger` are the same link written two ways, and
/// telling a candidate their own URL was "missing or altered" because the model
/// dropped the scheme is the false Critical this key exists to prevent. `www.`
/// is the same edit one label along — the model drops it (or adds it) exactly
/// as readily — and it drew TWO Criticals per link, one for the source form
/// "missing or altered" and one for the generated form "invented".
///
/// Only the FIRST `www.` label goes: `www.www.example.com` is a different host
/// from `www.example.com`, and this key may only ever collapse spellings of the
/// same resource.
///
/// **Never index a `&str` by a fixed byte offset here.** URL text in a résumé is
/// arbitrary UTF-8 (`www.café-berlin.de`, `ab.com/éx`), and `&s[..8]` panics the
/// moment a multibyte char straddles byte 8. Release builds are `panic = "abort"`,
/// so that panic killed the whole generation before the document was saved. Every
/// boundary this function cuts at is therefore either byte-compared on
/// `as_bytes()` (below) or derived from a char-aware API (`trim*`, `split_once`).
pub fn canonical_link(raw: &str) -> String {
    let mut s = raw.trim().trim_end_matches(['.', ',', ';', ':']);
    for scheme in ["https://", "http://"] {
        // Byte-compare the prefix; only slice once it is known to BE the scheme.
        if s.len() >= scheme.len()
            && s.as_bytes()[..scheme.len()].eq_ignore_ascii_case(scheme.as_bytes())
        {
            s = &s[scheme.len()..]; // Safe: the matched prefix is pure ASCII.
            break;
        }
    }
    // Same rule one label along, and the same byte-compare-then-slice shape:
    // `wwwé.de` is a host that merely starts with those three letters, and a
    // blind `&s[..4]` there cuts inside the `é` and aborts the process.
    const WWW: &str = "www.";
    if s.len() >= WWW.len() && s.as_bytes()[..WWW.len()].eq_ignore_ascii_case(WWW.as_bytes()) {
        s = &s[WWW.len()..]; // Safe: the matched prefix is pure ASCII.
    }
    let s = s.trim_end_matches('/');
    match s.split_once('/') {
        Some((host, path)) => format!("{}/{path}", host.to_lowercase()),
        None => s.to_lowercase(),
    }
}

/// The href inside a link entry, unwrapping a markdown span (`[label](href)`)
/// captured verbatim by `pipeline::resume::source::seed_one_project`. A bare
/// URL passes through unchanged.
///
/// Lives beside [`canonical_link`], not in `pipeline::resume`, because BOTH
/// `pipeline::resume::source` (which captures the span) and
/// `pipeline::resume::projects` (which compares it) need it, and putting it in
/// either one would make the other import from it — a module cycle. Every
/// canonical comparison either module makes routes through this first, so a
/// labeled and a bare copy of the same URL are never treated as two different
/// links.
pub fn link_href(link: &str) -> &str {
    let trimmed = link.trim();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return trimmed;
    };
    let Some(split) = rest.find("](") else {
        return trimmed;
    };
    let href = &rest[split + 2..];
    href.strip_suffix(')').unwrap_or(trimmed)
}

/// Whether a URL span names a RESOURCE rather than a package registry or a
/// library that happens to be host-shaped: it carries a scheme, a `www.` host,
/// or a path.
///
/// [`URL_RE`]'s third arm accepts a bare [`CODE_HOSTS`] name with no path
/// (`crates.io`, `npmjs.com`), which is correct on a title line — that is how a
/// project links to its package — and wrong on a stack line, where the same
/// token is the ecosystem the project is written for. A path or a scheme is
/// what tells the two apart; see [`project_links`].
///
/// `pub` and re-exported from [`super`] because the max-depth pipeline SEEDS a
/// project's links out of the same source section this check compares against:
/// a second answer to "is this span a link" there would mean the generator
/// omits a link the validator then reports as altered — a Critical produced by
/// two graders disagreeing rather than by anything wrong with the document.
pub fn names_a_resource(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("www.")
        || canonical_link(url).contains('/')
}

/// The links a projects section actually claims, verbatim.
///
/// The second line of an entry is the STACK line in the owner-locked projects
/// format ("Rust · SQLite · Clap"), and a technology list is never a link. What
/// it may still carry is a bare package-registry host, so the exclusion is
/// applied per URL — only a span that [`names_a_resource`] survives there.
///
/// It used to be applied per LINE, keyed on the literal `"://"`: a stack line
/// without that substring was dropped whole. A links line written without a
/// scheme ("github.com/janedoe/ledger", the form half of all résumés use) was
/// therefore cut out of the SOURCE link set, and a generated document that
/// spelled the same link out with `https://` was reported as linking somewhere
/// "not in your source résumé" — a Critical telling candidates they had invented
/// their own repository URL. Substring-testing for a scheme cannot tell a
/// missing scheme from a missing link; the resource test can.
///
/// Entry grouping is `consistency::project_entries`, so the two checks cannot
/// disagree about where an entry begins.
///
/// Returned PER ENTRY, keyed by [`project_entry_name`], because a link only
/// means "missing or altered" relative to an entry the document kept — see
/// [`project_link_issues`].
fn project_entry_links(section: &Section) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for entry in super::consistency::project_entries(section) {
        let mut urls = Vec::new();
        for (index, line) in entry.iter().enumerate() {
            let found = urls_in(&line.text);
            if index == 1 {
                urls.extend(found.into_iter().filter(|u| names_a_resource(u)));
            } else {
                urls.extend(found);
            }
        }
        out.push((project_entry_name(&entry), urls));
    }
    out
}

/// The identifying NAME of one projects entry: its title line up to the first
/// separator, reduced to lowercase alphanumeric words ("**Ledger CLI** · repo"
/// → `"ledger cli"`).
///
/// Deliberately an EXACT key rather than a fuzzy one. A stricter match makes
/// more entries look dropped, and a dropped entry is silent — so the error this
/// choice makes is a missed check, which is the direction this whole file errs
/// in. (A loose match would do the opposite: pair two unrelated projects and
/// report one's links as the other's.) An entry whose title line yields no
/// words is unmatchable and therefore silent too.
///
/// *Accepted cost, stated:* a generated document that RENAMES a project
/// ("Ledger CLI" → "Ledger CLI Tool") reads as one entry dropped and one added,
/// so the rename's links are no longer diffed against the original's — only
/// against the whole source set, which still catches an invented host.
fn project_entry_name(entry: &[&ParsedLine]) -> String {
    let Some(first) = entry.first() else {
        return String::new();
    };
    let head = first.text.split(['·', '|', '•']).next().unwrap_or_default();
    head.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<String>>()
        .join(" ")
}

/// `factual.altered_project_link` — the projects section's links must be the
/// candidate's own. Dropped, changed and invented all fire (a change surfaces as
/// one drop plus one invention).
///
/// A project link is how a reviewer verifies the work exists: a "helpfully"
/// corrected host or a stripped path leads them somewhere else. What is compared
/// is therefore the [`canonical_link`] key — everything that identifies the
/// resource — while the evidence quotes the span exactly as written, so the user
/// can see which of the two forms their document carries.
///
/// **Trimming is not altering, at either grain.** Cutting the section entirely
/// is a tailoring decision, and the commonest one there is — a résumé trimmed to
/// one page drops projects before it drops a role. Reading that as "every one of
/// your links was altered" produced a Critical per source link on a document
/// whose only change was an editorial cut, which is the loudest possible way to
/// be wrong about the safest possible edit.
///
/// The SAME argument applies one level down, and the section-level carve-out
/// alone did not cover it: trimming three projects to two is the identical edit
/// at the identical grain, and it fired a Critical per link on the entry that
/// went. So the source side reports only links belonging to an entry the
/// generated document KEPT ([`project_entry_name`]); a whole entry that is
/// simply gone is silent.
///
/// The GENERATED side is unscoped on purpose and needs no carve-out: trimming
/// removes links from that side, it never adds one, so a URL the source never
/// carried anywhere is an invention however it arrived.
///
/// Deliberately NOT replaced by a Warning-severity "dropped" note, at either
/// grain. A cut is not a defect: nothing was altered, invented or lost from the
/// candidate's own claims, so there is no evidence to show them and no action to
/// advise. If the cut actually cost the document something, that is a measured
/// finding `alignment.low_coverage` already makes — with the numbers to back it
/// — rather than a second unmeasured one here. (A new code would also need a
/// registered severity and an i18n key in both locales for a message that would
/// read "you did a normal thing".)
fn project_link_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    // ALL source Projects sections, unioned — not just the first. `SECTION_NAMES`
    // recognises both "projects" and "side projects", and both classify
    // `Projects`, so a second source Projects section is ordinary, not rare.
    // Reading only the first left the second one's links out of the sourced
    // set, so a document that never changed a thing accused itself of
    // inventing its own link — and unclearably: `criticals_by_section` routes
    // the finding to the FIRST Projects section, which never contained it.
    let mut source_sections = ctx
        .source_sections_of_kind(SectionKind::Projects)
        .peekable();
    if source_sections.peek().is_none() {
        return Vec::new(); // Nothing to compare against.
    }
    let source_entries: Vec<(String, Vec<String>)> =
        source_sections.flat_map(project_entry_links).collect();
    // ALL generated Projects sections, not just the first — see
    // `Analysis::generated_sections_of_kind`'s doc. An invented link that
    // lands in a SECOND Projects section (a duplicate the model produced, or
    // one already present in an imported résumé) must not go unchecked.
    let mut generated_sections = ctx
        .generated_sections_of_kind(SectionKind::Projects)
        .peekable();
    if generated_sections.peek().is_none() {
        return Vec::new(); // The section was cut, not rewritten — see above.
    }
    let generated_entries: Vec<(String, Vec<String>)> =
        generated_sections.flat_map(project_entry_links).collect();
    let flatten = |entries: &[(String, Vec<String>)]| -> Vec<String> {
        entries.iter().flat_map(|(_, u)| u.clone()).collect()
    };
    let source_urls = flatten(&source_entries);
    let generated_urls = flatten(&generated_entries);
    if source_urls.is_empty() && generated_urls.is_empty() {
        return Vec::new();
    }
    let key_set =
        |urls: &[String]| -> HashSet<String> { urls.iter().map(|u| canonical_link(u)).collect() };
    let source_keys = key_set(&source_urls);
    let generated_keys = key_set(&generated_urls);
    // An unnamed entry matches nothing, deliberately — see `project_entry_name`.
    let surviving: HashSet<&str> = generated_entries
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !name.is_empty())
        .collect();

    let mut issues = Vec::new();
    for url in source_entries
        .iter()
        .filter(|(name, _)| surviving.contains(name.as_str()))
        .flat_map(|(_, urls)| urls)
    {
        if !generated_keys.contains(&canonical_link(url)) {
            issues.push(issue(
                FACTUAL_ALTERED_PROJECT_LINK,
                Some("Projects"),
                format!(
                    "The project link {url} from your source résumé is missing or altered in \
                     the generated document. Project links must match your own exactly."
                ),
                Some(url.clone()),
            ));
        }
    }
    // `generated_urls` is the UNION of every generated Projects section
    // (`Analysis::generated_sections_of_kind`'s doc) — the same invented URL
    // can appear in two of them (a duplicate the model produced, or one
    // already present in an imported résumé), and without a seen-set each
    // occurrence pushed its own identical Critical. Deduped on the SAME
    // `canonical_link` key the comparison itself uses, so two spellings of one
    // invented resource still collapse to one finding — `HashSet::insert`
    // returning `false` on the second occurrence is exactly "already
    // reported".
    let mut reported_invented: HashSet<String> = HashSet::new();
    for url in &generated_urls {
        let key = canonical_link(url);
        if !source_keys.contains(&key) && reported_invented.insert(key) {
            issues.push(issue(
                FACTUAL_ALTERED_PROJECT_LINK,
                Some("Projects"),
                format!(
                    "The generated projects section links to {url}, which is not in your source \
                     résumé. Remove it or replace it with your own link."
                ),
                Some(url.clone()),
            ));
        }
    }
    issues
}

/// Recognised technical vocabulary — the only tokens `unsourced_term` polices.
///
/// A term counts as technical when it is a short tech acronym the keyword
/// kernel already allowlists, or either side of one of the kernel's synonym
/// pairs. Restricting to the kernel's own vocabulary keeps this from firing on
/// ordinary prose the model legitimately rephrased.
fn is_technical_term(token: &str) -> bool {
    SHORT_TECH_TERMS.contains(&token)
        || SYNONYMS
            .iter()
            .any(|(alias, canon)| *alias == token || *canon == token)
}

/// `factual.unsourced_term` — a technical skill the output claims that neither
/// the source résumé nor the posting mentions.
///
/// A Warning, not a Critical: the source résumé's own phrasing may simply have
/// used a different word for the same thing, and the posting is a legitimate
/// second source for vocabulary the candidate genuinely has.
fn unsourced_term_issues(generated: &str, truth_texts: &[&str]) -> Vec<ContentIssue> {
    let known: HashSet<String> = truth_texts
        .iter()
        .flat_map(|t| keywords_normalized(t))
        .collect();
    let mut terms: Vec<String> = keywords_normalized(generated)
        .into_iter()
        .filter(|t| is_technical_term(t))
        .filter(|t| !known.contains(t))
        .collect();
    terms.sort(); // Deterministic order — the token set is a HashSet.
    terms
        .into_iter()
        .map(|term| {
            issue(
                FACTUAL_UNSOURCED_TERM,
                None,
                format!(
                    "\"{term}\" appears in the generated document but in neither your source \
                     résumé nor the job ad. Keep it only if you can speak to it in an interview."
                ),
                Some(term),
            )
        })
        .collect()
}

/// Every factual check for a résumé, in a stable order.
pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = unsourced_metric_issues(
        ctx.input.generated,
        ctx.input.source_resume,
        DocKind::Resume,
    );
    issues.extend(dropped_role_issues(ctx));
    issues.extend(unsupported_date_issues(ctx));
    issues.extend(project_link_issues(ctx));
    issues.extend(unsourced_term_issues(
        ctx.input.generated,
        &[ctx.input.source_resume, ctx.input.job_ad],
    ));
    issues
}

/// The letter variant: a cover letter's truth base is the source résumé AND the
/// job ad (a letter legitimately quotes the posting's own numbers back), and it
/// has no roles, dates or projects section of its own to check.
pub(super) fn validate_letter(ctx: &Analysis) -> Vec<ContentIssue> {
    let truth = format!("{}\n{}", ctx.input.source_resume, ctx.input.job_ad);
    let mut issues = unsourced_metric_issues(ctx.input.generated, &truth, DocKind::CoverLetter);
    issues.extend(unsourced_term_issues(
        ctx.input.generated,
        &[ctx.input.source_resume, ctx.input.job_ad],
    ));
    issues
}
