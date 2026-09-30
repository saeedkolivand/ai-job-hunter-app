//! The pure "is this round worse" judgement: grouping Criticals by section to
//! regenerate, and the revert rule a candidate round is held to.

use std::collections::{BTreeMap, BTreeSet};

use crate::validate::content::{
    ContentIssue, ContentReport, CONSISTENCY_SKILL_NOT_DEMONSTRATED, CONTENT_LANGUAGE_MISMATCH,
    DUPLICATE_BULLET, FACTUAL_ALTERED_PROJECT_LINK, FACTUAL_DROPPED_ROLE,
};
use crate::validate::Severity;

use super::super::sections;
use super::super::validate::counts;

/// One issue rendered for the prompt: the code, the human message, and the
/// offending span. Content-bearing by necessity — this goes to the MODEL, not
/// to a log — and it rides inside `<section_issues>`, fenced like everything
/// else in the user turn.
///
/// `pub(super)` of `stages` (rather than private) so `humanize`'s
/// `predicates` — the SIBLING stage with the same "render one issue for the
/// model" need — reuses this exact format for its own `<humanize_findings>`
/// block instead of a second, driftable copy.
pub(in crate::pipeline::resume::stages) fn issue_line(issue: &ContentIssue) -> String {
    match &issue.evidence {
        Some(evidence) => format!(
            "[{}] {} — offending text: {evidence}",
            issue.code, issue.message
        ),
        None => format!("[{}] {}", issue.code, issue.message),
    }
}

/// Group a report's CRITICALS by the section that has to be regenerated,
/// WORST FIRST.
///
/// Two ways to locate one, in order:
///
/// 1. the validator's own `section` label, when it set one;
/// 2. otherwise, the section whose text CONTAINS the offending span.
///
/// The fallback is load-bearing, not defensive: the `factual.*` family — the
/// codes this loop exists for — reports `section: None` by design, because a
/// fabricated metric is found by comparing the document's numbers against the
/// source's, not by walking sections. Grouping on the label alone left the
/// commonest Critical unrepairable, which is the silent version of a repair
/// loop that does nothing.
///
/// A Critical that resolves to neither a label nor a containing section (a
/// finding in the leading band) is excluded: there is no section to
/// regenerate, and re-running one at random would not fix it. Those survive to
/// the terminal review.
///
/// **The document-wide `content.language_mismatch` Critical (`section: None`)
/// is excluded EXPLICITLY, before either lookup runs, not left to fall through
/// them.** Its evidence used to be the bare target-language code (`"de"`,
/// `"en"`), and [`sections::containing`]'s substring search matches those two
/// letters inside ordinary words (*der*, *Kunden*, *engineer*, *management*) —
/// so the fallback alone would route a whole-document translation failure to
/// whatever section happened to contain them, and `repair` would spend up to
/// `max_repair_attempts` provider calls regenerating that section against
/// "offending text: de", which can never clear a document-wide Critical. The
/// draft-stage retry (`Draft::run`) is this Critical's actual remedy; a
/// per-section language Critical still carries a `section` label and still
/// routes through [`sections::key_for_label`] as normal.
///
/// **The ORDER is the reason this returns a `Vec` rather than the `BTreeMap` it
/// builds.** A round can only afford `MAX_SECTIONS_PER_ROUND` sections, and a
/// map is ordered by wire key — `education` < `experience:0` < `projects` <
/// `skills` < `summary` — so a document with five failing sections would starve
/// `summary` deterministically, every round, forever. Worst-first (most
/// criticals, wire key as a stable tie-break) spends the budget where the
/// document is most wrong.
pub(crate) fn criticals_by_section(
    document: &str,
    report: &ContentReport,
) -> Vec<(String, Vec<String>)> {
    let split = sections::split(document);
    let lines: Vec<&str> = document.lines().collect();
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for issue in report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Critical)
    {
        // See the doc above: a document-wide language Critical has no section
        // to regenerate, and must not be let fall through to the
        // `sections::containing` substring fallback below.
        if issue.code == CONTENT_LANGUAGE_MISMATCH && issue.section.is_none() {
            continue;
        }
        let key = sections::key_for_label(issue.section.as_deref()).or_else(|| {
            let span = issue.evidence.as_deref()?;
            sections::containing(&split, &lines, span)
                .and_then(|section| sections::key_of(section.kind))
        });
        let Some(key) = key else { continue };
        grouped
            .entry(key.to_wire())
            .or_default()
            .push(issue_line(issue));
    }
    let mut ordered: Vec<(String, Vec<String>)> = grouped.into_iter().collect();
    // `sort_by_key` is stable, and the input came out of a BTreeMap, so equal
    // counts keep their wire-key order — deterministic without a second key.
    ordered.sort_by_key(|(_, issues)| std::cmp::Reverse(issues.len()));
    ordered
}

/// Whether a repair round's candidate must be discarded.
///
/// THREE terms, and the last two are not a bare count.
///
/// **The count term.** Strictly more criticals than the draft it was trying to
/// fix. The strictness is a decision with a gradient in both directions: `>=`
/// would abandon a round that traded one Critical for another — no ground lost,
/// and the budget's second round is exactly the chance to get it right — while
/// `after > before + n` would let a repair ship a document measurably worse
/// than the one it replaced.
///
/// **The ABSENCE term, and why a count alone was not enough.** A repair round
/// hands the model a whole section as free text and splices the answer back, so
/// it can lose content the source had. Losing content is not commensurable with
/// fixing a fabrication, and the count said it was: an assembled document
/// carrying TWO `factual.unsourced_metric` Criticals, "repaired" by a rewrite
/// that removed the invented figures and also dropped an employer, came back
/// with ONE `factual.dropped_role` — 1 < 2, an improvement by the only measure
/// the loop had. The round was kept, the document was saved, and an employer
/// the candidate actually worked for was gone from the résumé. Worse still, the
/// user could not undo it: an absence has no span, so it is deliberately not a
/// reviewable finding (see `commands::resume_pipeline::report::fabrications`) —
/// the run says "needs review" and the panel shows nothing to act on.
///
/// So a round that INTRODUCES an absence is worse whatever the totals say. The
/// comparison is by `(code, evidence)` PAIR, not by code: a document that
/// already lost a role must still be repairable (its own pair is carried, not
/// new), while a round that swaps WHICH employer is missing has introduced a
/// loss and is caught.
///
/// **The CROSS-SECTION terms, why they are a delta rather than a threshold,
/// and why the two codes are gated differently.** `duplicate.bullet` and
/// `consistency.skill_not_demonstrated` are the two Warning-level checks in
/// `validate/content` that reason ACROSS sections — see [`code_grew`] — and
/// repair acts on Criticals only, so nothing ever consumed them: a round that
/// doubled the whole document shipped 5 `duplicate.bullet` warnings, and a
/// round that deleted an employment entry shipped 4
/// `consistency.skill_not_demonstrated` warnings. An absolute "has any" gate
/// would also revert on a document that never regressed at all: rewording two
/// Experience bullets to drop one exact shared token — an ordinary section
/// rewrite — was measured to raise `skill_not_demonstrated` from zero to four
/// on an otherwise truthful document, and a calibrated non-zero threshold
/// would need recalibrating against that same noise forever. So both codes
/// ask the same question the absence term above does: not "does the document
/// carry this warning" but "did THIS ROUND grow it". A baseline that already
/// carries four is still repairable; only a round that carries MORE than its
/// own baseline is worse.
///
/// **`consistency.skill_not_demonstrated`'s delta alone still false-positives
/// on ordinary rewrite noise, so it is also GATED on the Critical count.**
/// "Did this round grow it" says nothing about WHY: the same ordinary-rewrite
/// noise that raises an already-elevated baseline is exactly what raises a
/// round's own before→after delta from zero the FIRST time that section is
/// ever touched — a round that took Criticals from five to zero while
/// rewording one Experience bullet enough to shift one shared token was
/// reverted on precisely that shape, discarding the fix the module doc's own
/// rule 4 says a Warning must never veto. So this ONE code's term only fires
/// when `criticals_of(after) >= criticals_of(before)` — it may sink a round
/// that bought nothing on the metric this whole module is scoped to, never
/// one that fixed what it was asked to fix.
///
/// **`duplicate.bullet` stays UNGATED.** The noise measurement above is about
/// `consistency.skill_not_demonstrated` specifically — an ordinary bullet
/// reword shifting a shared SKILL token — and was never observed for
/// `duplicate.bullet`, which fires on repeated bullet TEXT, not shared
/// vocabulary; an ordinary rewrite does not duplicate a bullet it also
/// rewords. `duplicate.bullet` is also the ONE signal that ever caught a
/// round doubling the whole document (the very first example above). Gating
/// it the same way as the skill-token code would veto exactly the round the
/// gate exists to catch: Criticals fixed, the whole document also duplicated
/// underneath the fix.
///
/// This is a compatible TIGHTENING of rule 4 in the module doc — that rule was
/// always "never hand back a worse document"; this says what the count could
/// not express. It fixes quality depth as well as max: quality's repair loop is
/// the same loop, and its draft carries the same employers.
pub(crate) fn round_is_worse(
    before: &ContentReport,
    before_text: &str,
    after: &ContentReport,
    after_text: &str,
) -> bool {
    let criticals_before = criticals_of(before);
    let criticals_after = criticals_of(after);
    if criticals_after > criticals_before {
        return true;
    }
    // A role count that fell is worse whatever the Critical totals say — the
    // ONE signal that saw a deleted employment entry even when the entry's own
    // company name was not distinctive/checkable enough for `factual.dropped_role`
    // to fire at all (see `factual::dropped_role_issues`'s own filter). Equal or
    // higher is not a regression; this only ever tightens the rule.
    if after.metrics.roles_output < before.metrics.roles_output {
        return true;
    }
    if coverage_dropped(before, after) {
        return true;
    }
    let carried = absences(before, before_text);
    if absences(after, after_text)
        .into_iter()
        .any(|pair| !carried.contains(&pair))
    {
        return true;
    }
    // UNGATED: see the doc above for why `duplicate.bullet` is never gated on
    // the Critical count.
    if code_grew(before, after, DUPLICATE_BULLET) {
        return true;
    }
    // Gated: `consistency.skill_not_demonstrated` may only veto a round that
    // bought NOTHING on Criticals. See the doc above for the false-positive
    // this closes.
    criticals_after >= criticals_before
        && code_grew(before, after, CONSISTENCY_SKILL_NOT_DEMONSTRATED)
}

/// Criticals in one report — the same count `super::super::validate::counts`
/// returns, without re-walking for the total.
fn criticals_of(report: &ContentReport) -> usize {
    counts(report).1
}

/// How many of `report`'s issues carry `code`.
fn code_count(report: &ContentReport, code: &str) -> usize {
    report.issues.iter().filter(|i| i.code == code).count()
}

/// Whether `after` carries more of `code` than `before` did — the shared core
/// [`round_is_worse`] applies once per cross-section code
/// (`duplicate.bullet`, `consistency.skill_not_demonstrated`), each under its
/// own gate. See `round_is_worse`'s own doc for why the two codes are gated
/// differently.
///
/// **Accepted blind spot, the Warning-side mirror of [`absences`]'s own:**
/// `code_count` reads `report.issues`, which is already the POST-`cap_issues`
/// list — `validate_content` truncates Warnings before Criticals once a
/// report is pinned at `MAX_CONTENT_ISSUES` (200) — so a `before` report
/// already at the cap can show a code undercounted against an uncapped
/// `after`, reading as growth that never happened. Reaching it needs a report
/// already pinned at 200 issues, the same reachability `absences` accepts for
/// the Critical side; this is the same deliberate choice, stated rather than
/// hidden, not a fix.
fn code_grew(before: &ContentReport, after: &ContentReport, code: &str) -> bool {
    code_count(after, code) > code_count(before, code)
}

/// Whether `after`'s keyword coverage fell by
/// `crate::validate::content::MIN_COVERAGE_DROP_POINTS` points or more below
/// `before`'s (the threshold itself counts as a drop, not just anything past
/// it) — the SAME points-of-drop threshold `alignment.low_coverage` already
/// reports at, reused rather than a second invented number.
///
/// Shared by [`round_is_worse`] (so `repair`'s up-to-`2 × MAX_SECTIONS_PER_ROUND`
/// blind per-section rewrites are held to the same coverage floor
/// `humanize`'s single whole-document rewrite always was) and, through it,
/// `super::super::humanize::predicates::humanize_is_worse` — one threshold,
/// read once, instead of a second copy that could drift from this one.
///
/// `None` on either side (an uncomparable posting — no extractable keywords,
/// see `crate::validate::content::ContentMetrics::keyword_coverage`) never
/// rejects: there is nothing to compare.
fn coverage_dropped(before: &ContentReport, after: &ContentReport) -> bool {
    match (
        before.metrics.keyword_coverage,
        after.metrics.keyword_coverage,
    ) {
        (Some(before), Some(after)) => {
            before - after >= crate::validate::content::MIN_COVERAGE_DROP_POINTS
        }
        _ => false,
    }
}

/// The ABSENCE-shaped Criticals in one report, as `(code, evidence)` pairs.
///
/// An absence-shaped finding names something the document is MISSING, so its
/// evidence is by definition not in the document — which is exactly why the
/// review panel cannot offer a verdict on one, and why a repair round is not
/// allowed to create one.
///
/// Two codes qualify, and the second only conditionally:
///
/// * `factual.dropped_role` — always. It names an employer the source has and
///   the output does not.
/// * `factual.altered_project_link` — only on its ABSENCE arm. That code is
///   emitted from two: a link the model INVENTED sits in the generated text (a
///   fabrication, reviewable, and a repair that produces one is caught by the
///   count like any other), while a SOURCE link missing or altered in the
///   output names a loss. The discriminator is the same one
///   `commands::resume_pipeline::report::fabrications` uses to keep that arm out
///   of the panel — is the evidence present in the document — so the two places
///   that decide "is this an absence" cannot disagree.
///
/// Scoped to those two rather than a general "evidence not in the text" gate,
/// for the reason `report::fabrications` records: `factual.unsourced_term`'s
/// evidence is a NORMALIZED token ("kubernetes" for a document that says
/// "Kubernetes"), so a blanket presence test would call ordinary fabrications
/// absences and freeze the repair loop.
///
/// **An issue with NO evidence is skipped**, because the pair is what makes a
/// pre-existing absence carryable rather than a permanent block. Both codes
/// always carry one — `factual.dropped_role`'s is the employer name it could
/// not find (`factual::dropped_roles` passes `Some(company)` unconditionally),
/// and the link check's is the URL — which is pinned by
/// `a_repair_rewrite_that_drops_a_seeded_employer_raises_a_dropped_role_critical`
/// rather than assumed here.
///
/// **Accepted blind spot, stated rather than hidden:** `validate_content` caps
/// a report at `MAX_CONTENT_ISSUES` (200) criticals-first, so a report holding
/// more than 200 Criticals could in principle have an absence truncated out of
/// it and this would not see it. Reaching that needs BOTH reports pinned at the
/// cap (the count term reverts anything that grows past it), i.e. a document
/// with 200+ deterministic Criticals — one already so broken that "which
/// finding got cut" is not the user's problem. The alternatives are a
/// pre-cap count field on `ContentReport` (a wire + persisted contract change)
/// or exempting absences from truncation (a second ordering rule inside the one
/// capper); neither is worth buying at that reachability, and this comment is
/// the deliberate choice rather than an oversight.
fn absences<'a>(report: &'a ContentReport, text: &str) -> BTreeSet<(&'a str, &'a str)> {
    report
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Critical)
        .filter_map(|issue| {
            let evidence = issue.evidence.as_deref()?.trim();
            let absent = issue.code == FACTUAL_DROPPED_ROLE
                || (issue.code == FACTUAL_ALTERED_PROJECT_LINK && !text.contains(evidence));
            absent.then_some((issue.code, evidence))
        })
        .collect()
}
