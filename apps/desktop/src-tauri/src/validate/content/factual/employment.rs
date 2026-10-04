//! `factual.dropped_role` and `factual.unsupported_date` — the employment
//! history a document keeps against the history its source states.

use std::collections::HashSet;

use crate::documents::evidence::{
    identity_tokens, split_entry, trailing_date_column, years_in, SectionKind, LEGAL_FORMS,
};
use crate::export::types::LineKind;
use crate::validate::content::{
    contains_phrase, issue, Analysis, ContentIssue, Section, FACTUAL_DROPPED_ROLE,
    FACTUAL_UNSUPPORTED_DATE,
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
pub(in crate::validate::content) fn count_roles(sections: &[Section]) -> usize {
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
pub(super) fn dropped_role_issues(ctx: &Analysis) -> Vec<ContentIssue> {
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
pub(super) fn unsupported_date_issues(ctx: &Analysis) -> Vec<ContentIssue> {
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
