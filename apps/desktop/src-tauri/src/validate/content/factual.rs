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
    identity_tokens, split_entry, years_in, SectionKind, LEGAL_FORMS, PRESENT_MARKERS,
};
use crate::documents::keywords::{keywords_normalized, SHORT_TECH_TERMS, SYNONYMS};
use crate::export::types::LineKind;

use super::{
    contains_phrase, has_real_contact_match, issue, Analysis, ContentIssue, Section,
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
    /// `1,200` / `4500` — three digits or more, never a 1900–2099 year.
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

/// Extract every impact metric from `text`, skipping the contact band.
///
/// Skipped deliberately:
/// * lines before the first section heading, and any line the parser classified
///   as `Contact`/`Name` that also carries a real address or phone — that is
///   where phone numbers and postal codes live, and neither is a claim about
///   impact. Nothing wider than that: the skip used to accept anything
///   `is_contact_shaped` matched, which is any line with two `·` separators or
///   the word "website", and a body bullet could evade the whole check by
///   containing one;
/// * 1900–2099 four-digit runs — those are years, checked by
///   [`unsupported_date_issues`] instead;
/// * numbers under three digits with no `%`/`x` unit.
pub fn metrics_in(text: &str) -> Vec<Metric> {
    let sections = super::split_sections(text);
    // A document with headings is a résumé: band 0 is name + contact, skip it
    // wholesale. A document without any is a letter: skip its letterhead by
    // shape instead (see [`MIN_WORDS_IN_LETTER_BODY_LINE`]).
    let has_headings = sections.len() > 1;
    let mut out = Vec::new();
    for (idx, section) in sections.iter().enumerate() {
        if has_headings && idx == 0 {
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
            // Skip the header band ONLY. The old test also skipped anything
            // `is_contact_shaped` accepted, which includes any line with two
            // `·` separators or the word "website" — so a body bullet reading
            // "Rebuilt the website · cut load time 90% · 4 releases" was exempt
            // from every metric check. A header line is one the parser
            // classified as such AND that carries a real address or phone.
            if matches!(line.kind, LineKind::Contact | LineKind::Name)
                && has_real_contact_match(&line.text)
            {
                continue;
            }
            collect_metrics(&line.text, &mut out);
        }
    }
    out
}

fn collect_metrics(line: &str, out: &mut Vec<Metric>) {
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
        let raw = caps[1].to_string();
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
fn unsourced_metric_issues(generated: &str, truth: &str) -> Vec<ContentIssue> {
    let sourced: HashSet<String> = metrics_in(truth)
        .into_iter()
        .map(|m| m.number)
        // Bare numbers, magnitude suffixes and word-numbers in the truth text
        // all count — see the doc comment.
        .chain(all_numbers(truth))
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
    metrics_in(generated)
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

/// Every number in `text`, normalized — the lenient half of the metric check.
fn all_numbers(text: &str) -> HashSet<String> {
    INTEGER_RE
        .captures_iter(text)
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

/// Magnitude-suffixed numbers in `text`, EXPANDED (`10k` → `10000`).
///
/// Only ever added to the SOURCE side's sourced set: a source that writes "10k
/// requests" and a generated bullet that writes "10,000 requests" state the same
/// fact, and the whole point of this pass is that a restatement is not a
/// fabrication.
fn suffixed_numbers(text: &str) -> HashSet<String> {
    SUFFIXED_NUMBER_RE
        .captures_iter(text)
        .filter_map(|caps| {
            let scale: f64 = match caps[2].to_ascii_lowercase().as_str() {
                "k" => 1_000.0,
                "m" => 1_000_000.0,
                _ => 1_000_000_000.0,
            };
            let value: f64 = normalize_number(&caps[1]).parse().ok()?;
            let expanded = value * scale;
            (expanded.fract() == 0.0 && expanded.is_finite()).then(|| format!("{expanded:.0}"))
        })
        .collect()
}

/// Words that state a multiplier without a digit. A source writing "doubled
/// throughput" and a generated bullet writing "2x throughput" are the same
/// claim; without this pair the restatement reads as an invented figure.
///
/// Deliberately tiny and one-directional (word → digits): these are the only
/// two multipliers a résumé states in words often enough to matter, and every
/// entry added here weakens a Critical, so the bar is "seen in real output".
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
    let generated_lower = ctx.input.generated.to_lowercase();
    entries(&ctx.source_sections)
        .into_iter()
        // Bound the scan before the expensive part: each surviving entry
        // substring-searches the whole generated document. See
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
/// Only years found inside a date-shaped context (a `JobEntry` line, or a line
/// carrying a present-tense marker at a WORD BOUNDARY) are considered, and only
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
/// The word-boundary rule on the markers matters just as much: `present` lives
/// inside "presented" and `now` inside "knowledge", so a substring test turned
/// ordinary bullets into date contexts and every year in them into a candidate
/// Critical.
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
            let lower = line.text.to_lowercase();
            let date_context = matches!(line.kind, LineKind::JobEntry)
                || PRESENT_MARKERS.iter().any(|m| contains_phrase(&lower, m));
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

/// The comparison key for a link: scheme dropped, host lowercased, one trailing
/// `/` removed. The PATH keeps its case — `/JaneDoe/Ledger` and `/janedoe/ledger`
/// are different resources on most hosts, and treating them as one would hide a
/// genuinely altered link.
///
/// Compared on the key, REPORTED verbatim. `https://github.com/janedoe/ledger`
/// and `github.com/janedoe/ledger` are the same link written two ways, and
/// telling a candidate their own URL was "missing or altered" because the model
/// dropped the scheme is the false Critical this key exists to prevent.
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
    let s = s.trim_end_matches('/');
    match s.split_once('/') {
        Some((host, path)) => format!("{}/{path}", host.to_lowercase()),
        None => s.to_lowercase(),
    }
}

/// The links a projects section actually claims, verbatim.
///
/// The `·`-separated STACK line is excluded structurally. In the owner-locked
/// projects format the links live on the title line and the second line of an
/// entry is the stack ("Rust · SQLite · Clap"); a technology list is never a
/// link, so it does not enter the comparison at all unless it carries an
/// explicit scheme. Entry grouping is `consistency::project_entries`, so the
/// two checks cannot disagree about where an entry begins.
fn project_links(section: &Section) -> Vec<String> {
    let mut out = Vec::new();
    for entry in super::consistency::project_entries(section) {
        for (index, line) in entry.iter().enumerate() {
            let is_stack_line = index == 1 && !line.text.contains("://");
            if !is_stack_line {
                out.extend(urls_in(&line.text));
            }
        }
    }
    out
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
/// **Both documents must HAVE a projects section.** Cutting the section
/// entirely is a tailoring decision, and the commonest one there is — a résumé
/// trimmed to one page drops projects before it drops a role. Reading that as
/// "every one of your links was altered" produced a Critical per source link on
/// a document whose only change was an editorial cut, which is the loudest
/// possible way to be wrong about the safest possible edit.
///
/// Deliberately NOT replaced by a Warning-severity "section dropped" note. A cut
/// section is not a defect: nothing was altered, invented or lost from the
/// candidate's own claims, so there is no evidence to show them and no action to
/// advise. If the cut actually cost the document something, that is a measured
/// finding `alignment.low_coverage` already makes — with the numbers to back it
/// — rather than a second unmeasured one here. (A new code would also need a
/// registered severity and an i18n key in both locales for a message that would
/// read "you did a normal thing".)
fn project_link_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source = ctx.source_section_of_kind(SectionKind::Projects);
    let Some(source) = source else {
        return Vec::new(); // Nothing to compare against.
    };
    let Some(generated) = ctx.section_of_kind(SectionKind::Projects) else {
        return Vec::new(); // The section was cut, not rewritten — see above.
    };
    let source_urls: Vec<String> = project_links(source);
    let generated_urls: Vec<String> = project_links(generated);
    if source_urls.is_empty() && generated_urls.is_empty() {
        return Vec::new();
    }
    let key_set =
        |urls: &[String]| -> HashSet<String> { urls.iter().map(|u| canonical_link(u)).collect() };
    let source_keys = key_set(&source_urls);
    let generated_keys = key_set(&generated_urls);

    let mut issues = Vec::new();
    for url in &source_urls {
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
    for url in &generated_urls {
        if !source_keys.contains(&canonical_link(url)) {
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
    let mut issues = unsourced_metric_issues(ctx.input.generated, ctx.input.source_resume);
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
    let mut issues = unsourced_metric_issues(ctx.input.generated, &truth);
    issues.extend(unsourced_term_issues(
        ctx.input.generated,
        &[ctx.input.source_resume, ctx.input.job_ad],
    ));
    issues
}
