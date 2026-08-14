//! `humanize` — the LAST stage, and the Warnings-side counterpart to
//! `repair`'s Criticals-only loop.
//!
//! ## Deterministic-first, exactly like `validate`
//!
//! Every `voice.*` finding on both documents is counted BEFORE a single
//! provider call is considered. Zero flags means the run is already clean by
//! the prompt's own bans, and this stage costs nothing — no call, no cache
//! lookup, nothing. A flagged document gets AT MOST ONE rewrite attempt
//! ([`humanize_one`]), never a loop: the model is asked to fix ONLY the
//! flagged lines, and the deterministic accept/revert rule below is what
//! actually decides whether the answer ships.
//!
//! ## The revert rule ([`humanize_is_worse`])
//!
//! Reuses [`super::repair::round_is_worse`] — the SAME "introduced a Critical
//! (code, evidence) pair the draft did not already carry" discipline `repair`
//! enforces — and adds one more way to lose: MORE `voice.*` flags than before.
//! A rewrite that traded one AI tell for two, or that fixed the flagged line by
//! inventing an unsourced number, is worse either way, and this stage's whole
//! job is to leave the document no worse than it found it.
//!
//! ## Never cached, one attempt, no loop
//!
//! Same reasoning as `repair`'s module doc: a cached correction to a document
//! that has since changed is the worst possible hit, and a model that cannot
//! fix a flagged line once will not fix it by being asked twice.
//!
//! ## Link lines are safe by construction, not by trust
//!
//! [`voice_findings`] drops any flagged line that also carries a URL BEFORE it
//! ever reaches the model, and the system prompt repeats the ban as a hard
//! contract. The résumé candidate additionally runs back through
//! [`projects::normalize_projects`] before it is graded — the SAME
//! deterministic, zero-cost pass `draft`/`repair` already run — so a rewrite
//! cannot silently alter a project link even if it tried to.
//!
//! ## Language residual
//!
//! A humanize rewrite in the target language (e.g., DE) using an English-lexicon
//! rule dictionary (antiAiTellProse) can seed English vocabulary into non-English
//! prose undetected by the per-language lexicon checks. Locale dispatch (using the
//! per-language prose checks, not the English ones) is the future fix.

use async_trait::async_trait;
use serde_json::json;

use crate::error::AppResult;
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::{humanize_system, humanize_user, HumanizeTier};
use crate::pipeline::resume::{projects, QualityCtx, RunDeadline};
use crate::pipeline::Stage;
use crate::validate::content::{ContentIssue, ContentMetrics, ContentReport};

use super::repair::issue_line;
use super::validate::validate_documents;

pub struct Humanize;

const NAME: &str = "humanize";

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
/// the same [`crate::validate::content::urls_in`] scan `factual` already uses
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

#[derive(Debug, Clone, Copy)]
pub enum HumanizeTierKind {
    Resume,
    Letter,
}

/// Whether a rewritten document is usable AT ALL — before it is ever graded.
///
/// Two things have to hold: non-empty (trimmed), and not drastically shorter
/// than the original. The length floor is tier-dependent:
/// - Resume tier: 50% (generous, accounts for trimmed wordy flagged bullets)
/// - Letter tier: 90% (strict, requires near-complete preservation)
///
/// What it catches is the shape `repair`'s own `sections::is_usable_replacement`
/// exists for: a truncated or refused answer that would otherwise be spliced in
/// as if it were the whole document, silently deleting everything past whatever
/// the model actually returned.
pub(crate) fn is_usable_rewrite(original: &str, candidate: &str, tier: HumanizeTierKind) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return false;
    }
    let original_len = original.trim().chars().count();
    if original_len == 0 {
        return true; // nothing to compare a ratio against
    }
    let candidate_len = candidate.chars().count();
    match tier {
        HumanizeTierKind::Resume => {
            // Resume: keep if at least 50% of original length
            candidate_len * 2 >= original_len
        }
        HumanizeTierKind::Letter => {
            // Letter: keep if at least 90% of original length (strict)
            candidate_len * 10 >= original_len * 9
        }
    }
}

/// Whether a humanize candidate must be discarded — [`super::repair::round_is_worse`]'s
/// Criticals/absence discipline, PLUS one more way to lose: more `voice.*`
/// flags than the document already carried. A rewrite that fixes one flagged
/// line by introducing two more has not improved the document, whatever the
/// Critical count says.
pub(crate) fn humanize_is_worse(
    before: &ContentReport,
    before_text: &str,
    after: &ContentReport,
    after_text: &str,
) -> bool {
    super::repair::round_is_worse(before, before_text, after, after_text)
        || voice_count(after) > voice_count(before)
}

/// What one document's humanize attempt did — everything the stage needs to
/// update `ctx` and record the ledger, so the attempt itself stays pure(-ish)
/// and testable with injected closures instead of a live `Completer`.
#[derive(Debug, Clone)]
pub(crate) struct HumanizeAttempt {
    /// The text to keep: the candidate on accept, the original on every other
    /// outcome.
    pub text: String,
    /// The report to keep, paired 1:1 with [`Self::text`].
    pub report: ContentReport,
    /// A provider call was actually made (charged and sent).
    pub called: bool,
    /// The candidate was graded and discarded as worse.
    pub reverted: bool,
    /// The provider call itself errored (network/provider failure) — distinct
    /// from a candidate that came back and was rejected.
    pub failed: bool,
    /// The run's deadline had already passed; nothing was attempted.
    pub timed_out: bool,
    /// The run hit a spend cap (Limiter refused the call) — distinct from
    /// a provider failure or a timeout.
    pub capped: bool,
}

impl HumanizeAttempt {
    fn kept(text: String, report: ContentReport) -> Self {
        Self {
            text,
            report,
            called: false,
            reverted: false,
            failed: false,
            timed_out: false,
            capped: false,
        }
    }
}

/// One document's whole humanize attempt, with the PROVIDER CALL, the
/// deterministic projects re-normalization, and the REVALIDATION all injected
/// — the same seam shape as [`super::repair::repair_loop`], and for the same
/// reason: the decisions here (deadline-first, empty-findings no-op,
/// usable-answer gate, accept/revert) must be provable by a test rather than
/// by reading the code, and this crate has no Tauri test harness to build a
/// real `Completer` from.
///
/// `findings` is ALREADY the filtered `<humanize_findings>` list
/// ([`voice_findings`]) — empty findings (every flag landed on a link line, or
/// there were none) is a no-op, not a call with nothing to ask about.
///
/// `normalize` is `|candidate| Option<String>`, exactly like `repair_loop`'s
/// own parameter: `Some` replaces the candidate with the re-rendered Projects
/// section, `None` means no change. The letter tier passes a closure that
/// always returns `None` — a letter has no Projects section to normalize.
pub(crate) async fn humanize_one<F, Fut, N, G, GFut>(
    deadline: RunDeadline,
    original_text: String,
    original_report: ContentReport,
    findings: Vec<String>,
    mut complete: F,
    normalize: N,
    mut revalidate: G,
    tier: HumanizeTierKind,
) -> AppResult<HumanizeAttempt>
where
    F: FnMut(String, Vec<String>) -> Fut,
    Fut: std::future::Future<Output = AppResult<String>>,
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: std::future::Future<Output = AppResult<ContentReport>>,
{
    if deadline.passed() {
        let mut attempt = HumanizeAttempt::kept(original_text, original_report);
        attempt.timed_out = true;
        return Ok(attempt);
    }
    if findings.is_empty() {
        return Ok(HumanizeAttempt::kept(original_text, original_report));
    }

    match complete(original_text.clone(), findings).await {
        Err(_) => {
            let mut attempt = HumanizeAttempt::kept(original_text, original_report);
            attempt.called = true;
            attempt.failed = true;
            Ok(attempt)
        }
        Ok(candidate) => {
            if !is_usable_rewrite(&original_text, &candidate, tier) {
                let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                attempt.called = true;
                return Ok(attempt);
            }
            let candidate = normalize(&candidate).unwrap_or(candidate);
            let candidate_report = revalidate(candidate.clone()).await?;
            if humanize_is_worse(
                &original_report,
                &original_text,
                &candidate_report,
                &candidate,
            ) {
                let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                attempt.called = true;
                attempt.reverted = true;
                Ok(attempt)
            } else {
                Ok(HumanizeAttempt {
                    text: candidate,
                    report: candidate_report,
                    called: true,
                    reverted: false,
                    failed: false,
                    timed_out: false,
                    capped: false,
                })
            }
        }
    }
}

/// The empty-but-valid report a letter revalidate falls back to when
/// [`validate_documents`] returns `None` for it — unreachable in practice
/// ([`is_usable_rewrite`] already guarantees the candidate is non-empty before
/// this runs, and a non-empty `generated` always yields `Some`), kept as a
/// defensive fallback rather than a panic so a future change to that contract
/// degrades instead of crashing a run.
fn empty_ok_report() -> ContentReport {
    ContentReport {
        ok: true,
        issues: Vec::new(),
        metrics: ContentMetrics::default(),
    }
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Humanize {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let Some(resume_report) = ctx.report.clone() else {
            // validate did not run — nothing to grade against.
            ctx.ledger.record(
                NAME,
                json!({
                    "resumeFlagged": 0,
                    "letterFlagged": 0,
                    "calls": 0,
                    "reverted": false,
                    "voiceBefore": 0,
                    "voiceAfter": 0,
                    "failed": false,
                    "timedOut": false,
                    "capped": false,
                }),
            );
            return Ok(());
        };
        let resume_flagged = voice_count(&resume_report);
        let letter_flagged = ctx.letter_report.as_ref().map_or(0, voice_count);
        let voice_before = resume_flagged + letter_flagged;

        if resume_flagged == 0 && letter_flagged == 0 {
            ctx.ledger.record(
                NAME,
                json!({
                    "resumeFlagged": 0,
                    "letterFlagged": 0,
                    "calls": 0,
                    "reverted": false,
                    "voiceBefore": 0,
                    "voiceAfter": 0,
                    "failed": false,
                    "timedOut": false,
                    "capped": false,
                }),
            );
            return Ok(());
        }

        let input = ctx.input;
        let completer = ctx.completer_for(NAME);
        // Computed once, exactly like `Draft::run`'s and `Repair::run`'s own
        // per-run seeding — every candidate reads the same seeds.
        let (seeds, _seed_skip_reason) = projects::seed_projects_for_normalize(input.source_resume);

        let mut calls: u32 = 0;
        let mut reverted = false;
        let mut failed = false;
        let mut timed_out = false;
        let mut capped = false;

        // Read out of `ctx` BEFORE building any closure — `input` is `Copy`,
        // `completer` is an owned `&'a Completer`, and this is the letter text
        // the resume's revalidate pass must check alongside it. Mirrors
        // `Repair::run`'s own reasoning: a closure that borrowed `ctx` itself
        // would still be alive (via `humanize_one`'s `.await`) when `ctx.draft`
        // is written below, which the borrow checker rightly refuses.
        let letter_for_resume_revalidate = ctx.letter_text().to_string();

        if resume_flagged > 0 {
            // Charge BEFORE entering humanize_one, so a cap refusal doesn't
            // count as a call or failure.
            match completer.charge_daily() {
                Err(_) => {
                    // Limiter refused — don't attempt the rewrite, just record
                    // the cap without charging count_call.
                    capped = true;
                }
                Ok(()) => {
                    let findings = voice_findings(&resume_report, &ctx.draft);
                    let attempt = humanize_one(
                        ctx.deadline,
                        ctx.draft.clone(),
                        resume_report,
                        findings,
                        |text, findings| async move {
                            completer
                                .complete(
                                    &humanize_system(HumanizeTier::Resume, input.target_language),
                                    &humanize_user(&text, &findings),
                                    None,
                                )
                                .await
                        },
                        |candidate: &str| projects::normalize_projects(candidate, &seeds),
                        |candidate| {
                            let letter = letter_for_resume_revalidate.clone();
                            async move {
                                let (report, _letter_report) = validate_documents(
                                    candidate,
                                    input.source_resume.to_string(),
                                    input.job_ad.to_string(),
                                    input.top_requirements.to_vec(),
                                    input.target_language.to_string(),
                                    letter,
                                )
                                .await?;
                                Ok(report)
                            }
                        },
                        HumanizeTierKind::Resume,
                    )
                    .await?;
                    calls += u32::from(attempt.called);
                    failed |= attempt.failed;
                    reverted |= attempt.reverted;
                    timed_out |= attempt.timed_out;
                    capped |= attempt.capped;
                    ctx.draft = attempt.text;
                    ctx.report = Some(attempt.report);
                }
            }
        }

        let letter_text = ctx.letter_text().to_string();
        if letter_flagged > 0 && !letter_text.trim().is_empty() && input.include_cover_letter {
            // Charge BEFORE entering humanize_one, so a cap refusal doesn't
            // count as a call or failure.
            match completer.charge_daily() {
                Err(_) => {
                    // Limiter refused — don't attempt the rewrite.
                    capped = true;
                }
                Ok(()) => {
                    // Safe: `letter_flagged > 0` only counts when `ctx.letter_report`
                    // is `Some` (see its own `voice_count` above).
                    let letter_report = ctx.letter_report.clone().unwrap_or_else(empty_ok_report);
                    let findings = voice_findings(&letter_report, &letter_text);
                    let draft_for_revalidate = ctx.draft.clone();
                    let attempt = humanize_one(
                        ctx.deadline,
                        letter_text,
                        letter_report,
                        findings,
                        |text, findings| async move {
                            completer
                                .complete(
                                    &humanize_system(HumanizeTier::Letter, input.target_language),
                                    &humanize_user(&text, &findings),
                                    None,
                                )
                                .await
                        },
                        // A letter has no Projects section to re-render.
                        |_candidate: &str| None,
                        |candidate| {
                            let draft = draft_for_revalidate.clone();
                            async move {
                                let (_resume_report, letter_report) = validate_documents(
                                    draft,
                                    input.source_resume.to_string(),
                                    input.job_ad.to_string(),
                                    input.top_requirements.to_vec(),
                                    input.target_language.to_string(),
                                    candidate,
                                )
                                .await?;
                                Ok(letter_report.unwrap_or_else(empty_ok_report))
                            }
                        },
                        HumanizeTierKind::Letter,
                    )
                    .await?;
                    calls += u32::from(attempt.called);
                    failed |= attempt.failed;
                    reverted |= attempt.reverted;
                    timed_out |= attempt.timed_out;
                    capped |= attempt.capped;
                    ctx.letter = attempt.text;
                    ctx.letter_report = Some(attempt.report);
                }
            }
        }

        let voice_after = ctx.report.as_ref().map_or(0, voice_count)
            + ctx.letter_report.as_ref().map_or(0, voice_count);

        if timed_out {
            // First-writer-wins: a run already cancelled or already out of
            // time upstream keeps its own reason.
            ctx.ledger.stop(StoppedReason::RunTimeout);
        }
        for _ in 0..calls {
            ctx.ledger.count_call(false);
        }
        // Counts only (ADR-027) — never the generated text or the finding
        // lines the model saw.
        ctx.ledger.record(
            NAME,
            json!({
                "resumeFlagged": resume_flagged,
                "letterFlagged": letter_flagged,
                "calls": calls,
                "reverted": reverted,
                "voiceBefore": voice_before,
                "voiceAfter": voice_after,
                "failed": failed,
                "timedOut": timed_out,
                "capped": capped,
            }),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_usable_rewrite_resume_tier_uses_50_percent_floor() {
        let original = "This is a substantial piece of text with multiple sentences.";
        let candidate_50_percent = "This is a substantial piece of"; // exactly 50%
        let candidate_49_percent = "This is a substantial piec";      // < 50%

        assert!(
            is_usable_rewrite(original, candidate_50_percent, HumanizeTierKind::Resume),
            "Resume tier should accept exactly 50% of original"
        );
        assert!(
            !is_usable_rewrite(original, candidate_49_percent, HumanizeTierKind::Resume),
            "Resume tier should reject less than 50% of original"
        );
    }

    #[test]
    fn is_usable_rewrite_letter_tier_uses_90_percent_floor() {
        let original = "This is a comprehensive cover letter with multiple paragraphs and complete thoughts.";
        // 90% of 82 chars is ~73.8, so 74 chars should pass, 72 should fail
        let candidate_90_percent = "This is a comprehensive cover letter with multiple paragraphs and complete thoug"; // ~78 chars, > 90%
        let candidate_60_percent = "This is a comprehensive cover letter with multiple"; // ~48 chars, < 90%

        assert!(
            is_usable_rewrite(original, candidate_90_percent, HumanizeTierKind::Letter),
            "Letter tier should accept 90%+ of original"
        );
        assert!(
            !is_usable_rewrite(original, candidate_60_percent, HumanizeTierKind::Letter),
            "Letter tier should reject 60% (below 90% threshold)"
        );
    }

    #[test]
    fn humanize_attempt_default_values() {
        let text = "test".to_string();
        let report = ContentReport {
            ok: true,
            issues: Vec::new(),
            metrics: ContentMetrics::default(),
        };
        let attempt = HumanizeAttempt::kept(text, report);

        assert!(!attempt.called, "kept() should have called=false");
        assert!(!attempt.reverted, "kept() should have reverted=false");
        assert!(!attempt.failed, "kept() should have failed=false");
        assert!(!attempt.timed_out, "kept() should have timed_out=false");
        assert!(!attempt.capped, "kept() should have capped=false");
    }
}
