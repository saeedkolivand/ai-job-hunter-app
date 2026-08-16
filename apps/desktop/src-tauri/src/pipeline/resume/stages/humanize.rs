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
//!
//! ## Shape, not just content (the `<humanize_document>` leak)
//!
//! A real run shipped an exported résumé with `<humanize_document>` as the
//! candidate's name and `</humanize_document>` as its last line: the model
//! returned the document WRAPPED in the fence tag `humanize_user` wraps it in
//! before sending it, and nothing checked for that. Every guard that already
//! existed — [`is_usable_rewrite`]'s length floor, [`humanize_is_worse`]'s
//! Critical/voice-flag comparison — grades CONTENT, and a wrapper only ADDS
//! length and introduces no new finding, so the corrupt candidate sailed
//! through both clean. `is_usable_rewrite` now also rejects any candidate
//! containing a registered fence tag
//! ([`crate::prompt_fence::contains_fence_tag`], checked against the whole
//! [`crate::prompt_fence`] registry, not just this one tag) — a REVERT, not a
//! strip-and-keep: a model that echoed the wrapper may have echoed other
//! scaffolding too, and this stage's job is cosmetic polish, so its failure
//! mode must be "no improvement", never "corrupted document".

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::prompts::{
    humanize_system, humanize_user, HumanizeTier, HUMANIZE_DOCUMENT_CAP,
};
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

/// Whether `document` is too large for `humanize` to safely rewrite as a
/// WHOLE document — the same cap [`humanize_user`]'s own `fenced()` call
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
/// Takes [`HumanizeTier`] — the SAME type [`humanize_system`] is built from,
/// not a parallel enum of this module's own. One type means one value flows to
/// both the prompt and the floor at each call site (see [`Humanize::run`]),
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
        || coverage_dropped(before, after)
}

/// Whether `after`'s keyword coverage fell by
/// [`crate::validate::content::MIN_COVERAGE_DROP_POINTS`] points or more below
/// `before`'s (the threshold itself counts as a drop, not just anything past
/// it) —
/// the SAME points-of-drop threshold `alignment.low_coverage` already reports
/// at, reused rather than a second invented number. Neither of
/// [`humanize_is_worse`]'s other two checks looks at keyword coverage at
/// all, so a rewrite that quietly deletes exact job-ad terms sailed through
/// both clean before this: no new Critical (deleting a term is not an
/// absence-shaped fabrication) and no new `voice.*` flag (coverage and voice
/// are unrelated checks).
///
/// `None` on either side (an uncomparable posting — no extractable keywords,
/// see [`crate::validate::content::ContentMetrics::keyword_coverage`]) never
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
    /// `original_text` was over [`HUMANIZE_DOCUMENT_CAP`] — see
    /// [`exceeds_humanize_cap`]. Nothing was sent; a truncated prefix rewrite
    /// is never an acceptable substitute for the whole document.
    pub too_large: bool,
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
            too_large: false,
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
    tier: HumanizeTier,
) -> AppResult<HumanizeAttempt>
where
    F: FnMut(String, Vec<String>) -> Fut,
    Fut: std::future::Future<Output = AppResult<String>>,
    N: Fn(&str) -> Option<String>,
    G: FnMut(String) -> GFut,
    GFut: std::future::Future<Output = AppResult<ContentReport>>,
{
    if exceeds_humanize_cap(&original_text) {
        let mut attempt = HumanizeAttempt::kept(original_text, original_report);
        attempt.too_large = true;
        return Ok(attempt);
    }
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
            // A revalidate failure (a `spawn_blocking` join failure inside
            // `validate_documents` — the process, not the model) is a FAILED
            // ATTEMPT, exactly like a `complete()` error above: keep the
            // original, mark `failed`, never propagate. Before this, the `?`
            // here was the ONE path through `humanize_one` that could fail
            // the WHOLE pipeline run over what is, by design, this stage's
            // own best-effort cleanup pass.
            let candidate_report = match revalidate(candidate.clone()).await {
                Ok(report) => report,
                Err(_) => {
                    let mut attempt = HumanizeAttempt::kept(original_text, original_report);
                    attempt.called = true;
                    attempt.failed = true;
                    return Ok(attempt);
                }
            };
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
                    too_large: false,
                })
            }
        }
    }
}

/// The empty-but-valid report used ONLY as the "before" baseline when
/// `ctx.letter_report` is somehow `None` even though `letter_flagged > 0` —
/// unreachable in practice (`voice_count`'s own `map_or(0, ..)` is what makes
/// `letter_flagged` positive, and that requires `Some`), kept as a defensive
/// fallback rather than a panic so a future change to that invariant degrades
/// instead of crashing a run.
///
/// **Safe as a BASELINE, wrong as an OUTCOME.** A fabricated clean "before"
/// only makes [`humanize_is_worse`]'s comparison MORE likely to revert (any
/// real issue in the candidate now reads as newly introduced), which is the
/// safe direction. Do not reuse this for a revalidate result: a fabricated
/// clean "after" report is what let an ungraded letter look accepted — see
/// the revalidate closure below, which fails CLOSED (an `Err`, caught by
/// `humanize_one`'s revalidate-error path) instead of calling this on `None`.
fn empty_ok_report() -> ContentReport {
    ContentReport {
        ok: true,
        issues: Vec::new(),
        metrics: ContentMetrics::default(),
    }
}

/// The stage's ledger artifact — one shape, shared by all three exits
/// (`Stage::run`'s two early returns and its normal end), so a future new
/// field lands in every exit at once instead of being added to the "real"
/// one and forgotten on the early-return copies. `Default` gives the two
/// early exits an all-zero/all-false artifact for free.
#[derive(Debug, Default)]
struct Artifact {
    resume_flagged: usize,
    letter_flagged: usize,
    calls: u32,
    reverted: bool,
    voice_before: usize,
    voice_after: usize,
    failed: bool,
    timed_out: bool,
    capped: bool,
    too_large: bool,
}

impl Artifact {
    // Counts only (ADR-027) — never the generated text or the finding lines
    // the model saw.
    fn into_json(self) -> Value {
        json!({
            "resumeFlagged": self.resume_flagged,
            "letterFlagged": self.letter_flagged,
            "calls": self.calls,
            "reverted": self.reverted,
            "voiceBefore": self.voice_before,
            "voiceAfter": self.voice_after,
            "failed": self.failed,
            "timedOut": self.timed_out,
            "capped": self.capped,
            "tooLarge": self.too_large,
        })
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
            ctx.ledger.record(NAME, Artifact::default().into_json());
            return Ok(());
        };
        let resume_flagged = voice_count(&resume_report);
        let letter_flagged = ctx.letter_report.as_ref().map_or(0, voice_count);
        let voice_before = resume_flagged + letter_flagged;

        if resume_flagged == 0 && letter_flagged == 0 {
            ctx.ledger.record(NAME, Artifact::default().into_json());
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
        let mut too_large = false;

        // Read out of `ctx` BEFORE building any closure — `input` is `Copy`,
        // `completer` is an owned `&'a Completer`, and this is the letter text
        // the resume's revalidate pass must check alongside it. Mirrors
        // `Repair::run`'s own reasoning: a closure that borrowed `ctx` itself
        // would still be alive (via `humanize_one`'s `.await`) when `ctx.draft`
        // is written below, which the borrow checker rightly refuses.
        let letter_for_resume_revalidate = ctx.letter_text().to_string();
        // Same reason, same timing: the RESOLVED list
        // (`QualityCtx::top_requirements`'s doc), read once before `ctx.draft`
        // starts getting rewritten below.
        let top_requirements = ctx.top_requirements();

        if resume_flagged > 0 {
            // The SAME three gates `humanize_one` itself checks first, mirrored
            // HERE so `charge_daily` — the call that actually spends the
            // user's daily allowance — never fires on a path that was never
            // going to send anything: a document over the cap (`fenced()`
            // would silently truncate it — see `exceeds_humanize_cap`), an
            // already-expired deadline, or every flagged line landing on a
            // link line (`voice_findings` filters them all out).
            if exceeds_humanize_cap(&ctx.draft) {
                too_large = true;
            } else if ctx.deadline.passed() {
                timed_out = true;
            } else {
                let findings = voice_findings(&resume_report, &ctx.draft);
                if !findings.is_empty() {
                    match completer.charge_daily() {
                        // Limiter refused — don't attempt the rewrite. Neither
                        // `called` nor `failed`: nothing was sent.
                        Err(_) => capped = true,
                        Ok(()) => {
                            // ONE value, used for both the prompt tier AND the
                            // usable-rewrite floor below — see
                            // `is_usable_rewrite`'s own doc for why a single
                            // `HumanizeTier` (not two independent literals) is
                            // what makes "letter prompt, résumé floor" a type
                            // a caller cannot construct.
                            let tier = HumanizeTier::Resume;
                            let attempt = humanize_one(
                                ctx.deadline,
                                ctx.draft.clone(),
                                resume_report,
                                findings,
                                |text, findings| async move {
                                    completer
                                        .complete(
                                            &humanize_system(tier, input.target_language),
                                            &humanize_user(&text, &findings),
                                            None,
                                        )
                                        .await
                                },
                                |candidate: &str| projects::normalize_projects(candidate, &seeds),
                                |candidate| {
                                    let letter = letter_for_resume_revalidate.clone();
                                    let top_requirements = top_requirements.clone();
                                    async move {
                                        let (report, _letter_report) = validate_documents(
                                            candidate,
                                            input.source_resume.to_string(),
                                            input.job_ad.to_string(),
                                            top_requirements,
                                            input.target_language.to_string(),
                                            letter,
                                        )
                                        .await?;
                                        Ok(report)
                                    }
                                },
                                tier,
                            )
                            .await?;
                            calls += u32::from(attempt.called);
                            failed |= attempt.failed;
                            reverted |= attempt.reverted;
                            timed_out |= attempt.timed_out;
                            too_large |= attempt.too_large;
                            ctx.draft = attempt.text;
                            ctx.report = Some(attempt.report);
                        }
                    }
                }
                // else: every flag landed on a link line — nothing to ask
                // about, `ctx.report` stays `resume_report`'s own content.
            }
        }

        // `ctx.letter` DIRECTLY — never `ctx.letter_text()`'s fallback. See
        // `should_humanize_letter`'s own doc for why the field-read has to be
        // this, not the "whichever letter is in scope" convenience accessor
        // every OTHER reader (validate, repair, persist) correctly uses.
        let letter_body = ctx.letter.clone();
        if should_humanize_letter(letter_flagged, &letter_body, input.include_cover_letter) {
            // Same three gates, same reason, as the résumé arm above.
            if exceeds_humanize_cap(&letter_body) {
                too_large = true;
            } else if ctx.deadline.passed() {
                timed_out = true;
            } else {
                // Safe: `letter_flagged > 0` only counts when `ctx.letter_report`
                // is `Some` (see its own `voice_count` above).
                let letter_report = ctx.letter_report.clone().unwrap_or_else(empty_ok_report);
                let findings = voice_findings(&letter_report, &letter_body);
                if !findings.is_empty() {
                    match completer.charge_daily() {
                        Err(_) => capped = true,
                        Ok(()) => {
                            let tier = HumanizeTier::Letter;
                            let draft_for_revalidate = ctx.draft.clone();
                            let attempt = humanize_one(
                                ctx.deadline,
                                letter_body,
                                letter_report,
                                findings,
                                |text, findings| async move {
                                    completer
                                        .complete(
                                            &humanize_system(tier, input.target_language),
                                            &humanize_user(&text, &findings),
                                            None,
                                        )
                                        .await
                                },
                                // A letter has no Projects section to re-render.
                                |_candidate: &str| None,
                                |candidate| {
                                    let draft = draft_for_revalidate.clone();
                                    let top_requirements = top_requirements.clone();
                                    async move {
                                        let (_resume_report, letter_report) = validate_documents(
                                            draft,
                                            input.source_resume.to_string(),
                                            input.job_ad.to_string(),
                                            top_requirements,
                                            input.target_language.to_string(),
                                            candidate,
                                        )
                                        .await?;
                                        // FAIL CLOSED: `None` here means a
                                        // non-empty candidate produced no
                                        // letter report at all (unreachable
                                        // today — see `empty_ok_report`'s own
                                        // doc). An `Err` is caught by
                                        // `humanize_one`'s revalidate-error
                                        // path (kept original, `failed`), so
                                        // a future contract change on that
                                        // `None` arm REVERTS instead of
                                        // shipping an ungraded letter under a
                                        // fabricated clean report.
                                        letter_report.ok_or_else(|| {
                                            AppError::Validation(
                                                "the letter revalidate produced no report for a \
                                                 non-empty candidate"
                                                    .to_string(),
                                            )
                                        })
                                    }
                                },
                                tier,
                            )
                            .await?;
                            calls += u32::from(attempt.called);
                            failed |= attempt.failed;
                            reverted |= attempt.reverted;
                            timed_out |= attempt.timed_out;
                            too_large |= attempt.too_large;
                            ctx.letter = attempt.text;
                            ctx.letter_report = Some(attempt.report);
                        }
                    }
                }
                // else: every flag landed on a link line — nothing to ask
                // about, `ctx.letter`/`ctx.letter_report` stay as they are.
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
        ctx.ledger.record(
            NAME,
            Artifact {
                resume_flagged,
                letter_flagged,
                calls,
                reverted,
                voice_before,
                voice_after,
                failed,
                timed_out,
                capped,
                too_large,
            }
            .into_json(),
        );
        Ok(())
    }
}
