//! Factual grounding — the Critical family.
//!
//! Every check here compares the generated text against the candidate's own
//! source document(s). Nothing in this file consults a model, and nothing
//! guesses: if a comparison cannot be made reliably it is skipped, because a
//! false "you fabricated this" is worse than a missed one.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::{split_entry, years_in, SectionKind, PRESENT_MARKERS};
use crate::documents::keywords::{keywords_normalized, SHORT_TECH_TERMS, SYNONYMS};
use crate::export::parser::is_contact_shaped;
use crate::export::types::LineKind;

use super::{
    issue, Analysis, ContentIssue, Section, FACTUAL_ALTERED_PROJECT_LINK, FACTUAL_DROPPED_ROLE,
    FACTUAL_UNSOURCED_METRIC, FACTUAL_UNSOURCED_TERM, FACTUAL_UNSUPPORTED_DATE,
};

/// Minimum characters of a company token before it counts as distinctive
/// enough to decide a role went missing. "AG", "Inc" or "The" appearing
/// nowhere in the output proves nothing.
pub const MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS: usize = 4;

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
static MULTIPLIER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d[\d.,]*\d|\d)\s*[x×](?:\b|$)").unwrap());
/// Leading `\b` only, deliberately. Requiring a trailing boundary too would
/// miss `480ms`; dropping the leading one would make `sha256` yield "256".
static INTEGER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d[\d.,\u{202F}\u{00A0}]*\d|\d)").unwrap());

/// A heading-less document (a cover letter) has no section band to skip, so the
/// letterhead is skipped by shape instead: metrics are read only from the body,
/// which starts at the first line long enough to be a sentence. An address line
/// ("10115 Berlin") is short; a claim of impact never is.
pub const MIN_WORDS_IN_LETTER_BODY_LINE: usize = 8;

/// Normalize a written number to a comparable string: drop digit-grouping
/// separators (`1,200` / `1.200` / `1 200` → `1200`) and render a decimal
/// comma as a period (`3,5` → `3.5`).
///
/// A separator counts as GROUPING when exactly three digits follow it and more
/// digits follow those or the number ends there; otherwise it is a decimal
/// point. Locale-neutral by construction, which matters because the source and
/// the generated text can be written in different markets' conventions.
pub fn normalize_number(raw: &str) -> String {
    let digits_and_seps: Vec<char> = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '\u{202F}' | '\u{00A0}' | ' '))
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
/// * lines before the first section heading, and any contact-shaped line —
///   that is where phone numbers and postal codes live, and neither is a claim
///   about impact;
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
            if matches!(line.kind, LineKind::Contact | LineKind::Name)
                || is_contact_shaped(&line.text)
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
/// Deliberately lenient about WHERE the number appears in the source: it fires
/// only when the normalized NUMBER is absent from the source entirely, not when
/// the unit differs. A source writing "cut latency by 40 percent" and a
/// generated bullet writing "40%" are the same fact in different clothes, and
/// flagging that as fabrication is exactly the false positive that would make a
/// user stop trusting the whole report.
fn unsourced_metric_issues(generated: &str, truth: &str) -> Vec<ContentIssue> {
    let sourced: HashSet<String> = metrics_in(truth)
        .into_iter()
        .map(|m| m.number)
        // Bare numbers in the truth text also count — see the doc comment.
        .chain(all_numbers(truth))
        .collect();
    let mut seen = HashSet::new();
    metrics_in(generated)
        .into_iter()
        .filter(|m| !sourced.contains(&m.number))
        .filter(|m| seen.insert(m.raw.clone()))
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

/// Company tokens distinctive enough to decide whether an entry survived.
///
/// Lowercased alphanumeric tokens of `MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS`+
/// characters, minus legal-form suffixes that carry no identity.
fn distinctive_tokens(company: &str) -> Vec<String> {
    const LEGAL_FORMS: &[&str] = &[
        "gmbh",
        "corp",
        "corporation",
        "limited",
        "incorporated",
        "holding",
        "group",
        "company",
    ];
    company
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|t| t.chars().count() >= MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS)
        .filter(|t| !LEGAL_FORMS.contains(&t.as_str()))
        .collect()
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
/// An entry is identified by the pair (distinctive company tokens, year
/// tokens). It counts as DROPPED only when *none* of its distinctive company
/// tokens (see [`distinctive_tokens`]: 4+ characters, legal forms removed)
/// appears anywhere in the generated text — not merely when the dates moved or
/// the wording changed. An entry with no distinctive token at all is skipped
/// rather than guessed at.
///
/// That is deliberately the narrowest possible reading: shortening a role is a
/// legitimate tailoring decision, whereas a company vanishing from the document
/// entirely is the loss the candidate would never notice until an interviewer
/// did.
fn dropped_role_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let generated_lower = ctx.input.generated.to_lowercase();
    entries(&ctx.source_sections)
        .into_iter()
        .filter_map(|(company, dates)| {
            let tokens = distinctive_tokens(&company);
            if tokens.is_empty() {
                return None;
            }
            if tokens.iter().any(|t| generated_lower.contains(t)) {
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
/// carrying a present-tense marker) are considered, and only years the source
/// never states. Even then it fires only when
///
/// * the year is EARLIER than the latest year the source knows about — an
///   invented earlier date can never be a legitimate resolution of anything; or
/// * the source contains no open-ended span at all ("Present", "Heute", …), in
///   which case there is nothing for a later year to be resolving either.
///
/// The carve-out exists because a source that says `2021 – Present` and output
/// that says `2021 – 2026` is the same fact with the open end resolved, and
/// calling that fabrication would be wrong. Deterministic on purpose: the check
/// never reads the system clock.
fn unsupported_date_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source_years: HashSet<u32> = years_in(ctx.input.source_resume).into_iter().collect();
    let Some(&source_max) = source_years.iter().max() else {
        return Vec::new(); // No dates to compare against — stay quiet.
    };
    let source_lower = ctx.input.source_resume.to_lowercase();
    let source_open_ended = PRESENT_MARKERS.iter().any(|m| source_lower.contains(m));

    let mut seen = HashSet::new();
    let mut issues = Vec::new();
    for section in &ctx.generated_sections {
        for line in &section.lines {
            let lower = line.text.to_lowercase();
            let date_context = matches!(line.kind, LineKind::JobEntry)
                || PRESENT_MARKERS.iter().any(|m| lower.contains(m));
            if !date_context {
                continue;
            }
            let dates = line.right_text.as_deref().unwrap_or(&line.text);
            for year in years_in(dates) {
                if source_years.contains(&year) || !seen.insert(year) {
                    continue;
                }
                if year < source_max || !source_open_ended {
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

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:https?://[^\s)\]<>]+|(?:www\.)[^\s)\]<>]+|[a-z0-9-]+\.(?:com|org|net|io|dev|app|de|co|ai|sh|me)(?:/[^\s)\]<>]*)?)").unwrap()
});

/// Every URL in `text`, verbatim, with trailing sentence punctuation trimmed.
/// Markdown link targets are captured by the same pass — the regex matches the
/// href inside `[anchor](href)` as well as a bare URL.
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

/// `factual.altered_project_link` — the projects section's links must be the
/// candidate's own, verbatim. Dropped, changed and invented all fire (a change
/// surfaces as one drop plus one invention).
///
/// Verbatim on purpose: a project link is how a reviewer verifies the work
/// exists. A "helpfully" corrected host or a stripped path leads them to the
/// wrong place, and normalizing before comparing would hide exactly that.
fn project_link_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source = ctx.source_section_of_kind(SectionKind::Projects);
    let Some(source) = source else {
        return Vec::new(); // Nothing to compare against.
    };
    let section_text = |s: &Section| {
        s.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let source_urls: Vec<String> = urls_in(&section_text(source));
    let generated_urls: Vec<String> = ctx
        .section_of_kind(SectionKind::Projects)
        .map(|s| urls_in(&section_text(s)))
        .unwrap_or_default();
    if source_urls.is_empty() && generated_urls.is_empty() {
        return Vec::new();
    }

    let mut issues = Vec::new();
    for url in &source_urls {
        if !generated_urls.contains(url) {
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
        if !source_urls.contains(url) {
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
