//! The pure decisions `humanize` takes away from the model: what counts as a
//! `voice.*` finding, what the stage is even allowed to send, and whether a
//! candidate it got back is usable or must be reverted.

use crate::pipeline::resume::prompts::{HumanizeTier, HUMANIZE_DOCUMENT_CAP};
use crate::validate::content::{ContentIssue, ContentReport};

use super::super::repair::issue_line;

/// A `voice.*` finding — the whole Warnings family the generation prompt's
/// anti-AI-tell bans exist to check. Every OTHER code family (`factual.*`,
/// `ats.*`, `consistency.*`, `duplicate.*`) is out of scope for this stage on
/// purpose: fixing a fabrication is `repair`'s job, and it works from
/// Criticals only.
fn is_voice_issue(issue: &ContentIssue) -> bool {
    issue.code.starts_with("voice.")
}

/// How many `voice.*` findings one report carries — the gate ("is there
/// anything to do") and the ledger's `voiceBefore`/`voiceAfter`.
pub(crate) fn voice_count(report: &ContentReport) -> usize {
    report
        .issues
        .iter()
        .filter(|issue| is_voice_issue(issue))
        .count()
}

/// Whether `evidence` sits on a line of `document` that also carries a URL —
/// the same `crate::validate::content::urls_in` scan `factual` already uses
/// to find project links. A voice finding whose span happens to share a line
/// with a link must never become a rewrite instruction: the model is told the
/// same thing in the system prompt, but a finding that never reaches
/// `<humanize_findings>` cannot be touched even by a model that ignores the
/// rule.
fn on_link_line(document: &str, evidence: &str) -> bool {
    let evidence = evidence.trim();
    if evidence.is_empty() {
        return false;
    }
    document
        .lines()
        .any(|line| line.contains(evidence) && !crate::validate::content::urls_in(line).is_empty())
}

/// The `<humanize_findings>` list for one document: every `voice.*` finding,
/// rendered with [`issue_line`] (the SAME format `repair` sends the model),
/// minus any finding on a project-link line ([`on_link_line`]).
pub(crate) fn voice_findings(report: &ContentReport, document: &str) -> Vec<String> {
    report
        .issues
        .iter()
        .filter(|issue| is_voice_issue(issue))
        .filter(|issue| {
            !issue
                .evidence
                .as_deref()
                .is_some_and(|evidence| on_link_line(document, evidence))
        })
        .map(issue_line)
        .collect()
}

/// Whether `document` is too large for `humanize` to safely rewrite as a
/// WHOLE document — the same cap `humanize_user`'s own `fenced()` call
/// enforces on the way OUT (`HUMANIZE_DOCUMENT_CAP`), checked HERE on the way
/// IN so a document that fence would silently truncate is never sent at all.
///
/// **Why this cannot be left to [`is_usable_rewrite`]'s length floor.**
/// `fenced()` truncates with NO marker, so a document over the cap does not
/// error — the model is handed a PREFIX and asked to rewrite "the whole
/// document", and a faithful rewrite of that prefix is a candidate whose
/// length, measured against the REAL (untruncated) original, can still clear
/// the résumé's 50% floor: a 24 000-char résumé truncated to the 12 000-char
/// cap and rewritten faithfully comes back at ~50% of the original — right at
/// the boundary, and comfortably over it for anything shorter than double the
/// cap. Whether that passing candidate actually LOST content depends on what
/// was in the dropped tail: `humanize_is_worse`'s absence-shaped Criticals
/// only fire when the lost tail happened to contain a role or a project link,
/// so a dropped Education/Certifications/Languages section — real content —
/// sails through both guards clean. A length ratio against a document that
/// was never fully SEEN cannot be the backstop; refusing to send it at all
/// is.
pub(crate) fn exceeds_humanize_cap(document: &str) -> bool {
    document.chars().count() > HUMANIZE_DOCUMENT_CAP
}

/// Whether the letter arm may run at all — pulled out as its OWN pure
/// predicate, not inlined into an `if`, because this is the gate HIGH-1 exists
/// for: the letter arm must be structurally unadoptable whenever this run
/// never asked for a letter, independent of which text `letter_body` happens
/// to hold.
///
/// **Two conditions, both load-bearing, neither one alone is enough:**
///
/// * `include_cover_letter` — the run REQUESTED a letter. Without this alone,
///   a validate-only caller that hands `coverLetterText` in for checking (the
///   legacy path `QualityInput::cover_letter` still serves) would have its
///   text silently REWRITTEN by a stage it never asked to run, and — because
///   `persist_document` writes `cover_letter_text: ctx.letter.clone()` — the
///   humanized rewrite would then overwrite the posting's stored letter with
///   a document this run never generated.
/// * `!letter_body.trim().is_empty()` — there is something to rewrite.
///   `letter_body` MUST be `ctx.letter` (the `cover_letter` stage's own
///   output), never `ctx.letter_text()`'s fallback: reading the field
///   directly is what makes "no request, no rewrite" true by CONSTRUCTION —
///   `ctx.letter` is empty whenever `cover_letter` skipped, so a caller could
///   drop the `include_cover_letter` check entirely and this would still
///   refuse. Keeping both is defense in depth, not redundancy: the field-read
///   protects against a future change to the flag check, and the flag check
///   protects against a future change to what populates `ctx.letter`.
///
/// `letter_flagged` is checked by the caller before this — a letter with
/// nothing flagged has nothing to humanize regardless of the other two.
pub(crate) fn should_humanize_letter(
    letter_flagged: usize,
    letter_body: &str,
    include_cover_letter: bool,
) -> bool {
    letter_flagged > 0 && !letter_body.trim().is_empty() && include_cover_letter
}

/// Whether a rewritten document is usable AT ALL — before it is ever graded.
///
/// Three things have to hold: non-empty (trimmed), free of any registered
/// fence tag, and not drastically shorter than the original.
///
/// **The fence-tag check is a SHAPE gate, not a content one — the gap a real
/// incident found.** The humanize contract tells the model to return the FULL
/// document; a model that returns it WRAPPED in the `<humanize_document>` tag
/// it was handed produces a candidate that is non-empty, at or over the length
/// floor (a wrapper only ADDS length), and carries no new voice/factual
/// finding — every other guard in this stage grades content, and content was
/// fine. `crate::prompt_fence::contains_fence_tag` is what actually catches it,
/// checked against the FULL registry so any known tag — not just this one —
/// is caught, present and future.
///
/// **Revert, not repair, and deliberately so.** A model that echoed the fence
/// wrapper may have echoed other scaffolding too; stripping only the one tag
/// this stage knows about could leave subtler damage behind while looking
/// clean. Humanize is a cosmetic, best-effort stage — its failure mode must be
/// "no improvement", never "corrupted document" — so a shape-broken candidate
/// is treated exactly like an unusable/truncated one: discarded, original
/// kept, `called` still recorded so the attempt is not hidden.
///
/// The length floor is tier-dependent, and the asymmetry is deliberate, not a
/// stricter-is-safer default:
///
/// - **Resume tier: 50%**, generous — a rewrite that trims a wordy flagged
///   bullet is still legitimate, and `humanize_is_worse` has a REAL backstop
///   against content loss regardless: `round_is_worse`'s absence-shaped
///   Criticals (`factual.dropped_role`, `factual.altered_project_link`'s
///   absence arm) catch a rewrite that silently drops a role or a project
///   link, so the length floor only has to catch outright truncation.
/// - **Letter tier: 90%**, strict — a letter has NO absence-shaped validator.
///   Nothing in `validate::content::letter` names a paragraph, a company
///   detail, or a claim the letter USED TO make and no longer does; the
///   voice/factual checks it does run are span-shaped, not presence-shaped.
///   This length floor is therefore the letter's ONLY backstop against
///   content loss — a candidate that quietly drops a paragraph would
///   otherwise sail through revalidation clean.
///
/// What it catches is the shape `repair`'s own `sections::is_usable_replacement`
/// exists for: a truncated or refused answer that would otherwise be spliced in
/// as if it were the whole document, silently deleting everything past whatever
/// the model actually returned.
///
/// Takes [`HumanizeTier`] — the SAME type `humanize_system` is built from,
/// not a parallel enum of this module's own. One type means one value flows to
/// both the prompt and the floor at each call site (see `Humanize::run`),
/// so "wrote the letter prompt but graded it against the résumé's floor" is a
/// type a caller cannot construct, rather than a coincidence two independent
/// arguments happen to agree on today.
pub(crate) fn is_usable_rewrite(original: &str, candidate: &str, tier: HumanizeTier) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return false;
    }
    if crate::prompt_fence::contains_fence_tag(candidate) {
        return false;
    }
    let original_len = original.trim().chars().count();
    if original_len == 0 {
        return true; // nothing to compare a ratio against
    }
    let candidate_len = candidate.chars().count();
    match tier {
        HumanizeTier::Resume => {
            // Resume: keep if at least 50% of original length
            candidate_len * 2 >= original_len
        }
        HumanizeTier::Letter => {
            // Letter: keep if at least 90% of original length (strict)
            candidate_len * 10 >= original_len * 9
        }
    }
}

/// Whether a humanize candidate must be discarded — [`super::super::repair::round_is_worse`]'s
/// Criticals/role-count/coverage/absence/cross-section discipline, PLUS one
/// more way to lose: more `voice.*` flags than the document already carried. A rewrite
/// that fixes one flagged line by introducing two more has not improved the
/// document, whatever the Critical count says.
///
/// The coverage floor used to live here alone; it is now
/// `super::super::repair::coverage_dropped`, folded into `round_is_worse` itself
/// so `repair`'s own per-section rewrites are held to it too — this function
/// no longer names it as a separate clause.
pub(crate) fn humanize_is_worse(
    before: &ContentReport,
    before_text: &str,
    after: &ContentReport,
    after_text: &str,
) -> bool {
    super::super::repair::round_is_worse(before, before_text, after, after_text)
        || voice_count(after) > voice_count(before)
}
