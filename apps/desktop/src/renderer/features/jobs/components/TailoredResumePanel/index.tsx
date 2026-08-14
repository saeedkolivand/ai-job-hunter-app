import { FileText, Loader2, ShieldCheck, Sparkles, Square, X } from 'lucide-react';
import { useMemo, useRef, useState } from 'react';

import { AGENT_RESUME_TEXT_CAP, detectLanguage, type PipelineSectionKey } from '@ajh/shared';
import type { PipelineRunEvent } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, cn, ModalShell, Skeleton, StepDots, Tag } from '@ajh/ui';

import { PipelineRunsList } from '@/components/generation/PipelineRunsList';
import { QualityReportPanel } from '@/components/generation/QualityReportPanel';
import { ImproveResumeRun } from '@/features/jobs/components/ImproveResumeRun';
import { useAgentRunSession } from '@/features/jobs/hooks/useAgentRunSession';
import type { Posting } from '@/features/jobs/types';
import {
  type ResumePipelineSession,
  useResumePipelineSession,
} from '@/hooks/use-resume-pipeline-session';
import { useDefaultResume } from '@/hooks/useDefaultResumeId';
import { errorDetail } from '@/lib/error-class';
import { buildSectionVerdicts, parseFabrications, unresolvedCount } from '@/lib/generate';
import { stoppedSuffix } from '@/lib/stopped-reason';
import {
  useGenerateConfig,
  usePipelineRun,
  usePipelineRunsForJob,
  useRegenerateSection,
  useResolveFabrication,
} from '@/services';

/**
 * Longest generated résumé the `improve_resume` flow can review, in CHARACTERS
 * — `agent::tools::RESUME_CAP`, the cap the run's seed message fences the
 * generation at (the same one `validate_resume` reads a draft at).
 *
 * The backend REFUSES a longer one at run start rather than truncating: the
 * flow would review the first 8 000 characters and then offer that corrected
 * prefix through `save_resume`, whose own cap is 40 000 — an approve would
 * overwrite the document with its own truncated head. Checked here as well so
 * the action is withheld with a reason instead of failing on click; the stored
 * text is still the authority, so the run's own error is handled either way.
 *
 * One number: `AGENT_RESUME_TEXT_CAP` is the shared source the backend's
 * `RESUME_CAP` is generated from (`pnpm gen:ipc`), so the gate here and the
 * refusal there cannot drift.
 */
const GENERATION_REVIEW_CAP = AGENT_RESUME_TEXT_CAP;

export interface ImproveEligibility {
  /** The pipeline run being shown has ended (`session.state !== 'idle' && !busy`). */
  terminal: boolean;
  /** That run's own outcome. */
  runState: ResumePipelineSession['state'];
  /** A run RECORD is loaded (not just a session that started one). */
  hasDetail: boolean;
  /** The record carries résumé text — i.e. a generation exists for this posting. */
  hasDocument: boolean;
  /** The shown run is the posting's newest (`writable`, the one-document rule). */
  writable: boolean;
  /** A master résumé and a provider are configured. */
  canRunStaged: boolean;
}

/**
 * Whether "Improve this résumé" may be offered — extracted so the rule can be
 * tested term by term.
 *
 * It cannot be exercised through the panel alone: the report REPLACES the modal
 * (one dialog at a time), so every state that makes `writable` false also
 * unmounts the footer the button lives in, and a component test asserting "no
 * button" there passes whether or not the term is present. That is the
 * guard-shaped-decoration trap — the rule is asserted here instead.
 */
export function canImproveGeneration({
  terminal,
  runState,
  hasDetail,
  hasDocument,
  writable,
  canRunStaged,
}: ImproveEligibility): boolean {
  return (
    terminal &&
    // A run that produced a document. What a failed or stopped one leaves
    // behind is not a résumé to review.
    (runState === 'done' || runState === 'needsReview') &&
    hasDetail &&
    // `documentText` IS `find_for_job`'s record — exactly what the run resolves
    // server-side. No generation means the run would fail with "generate one
    // first".
    hasDocument &&
    // Every run of a posting merges into ONE saved résumé, and this flow's save
    // merges into that same one, so only the newest may be reviewed.
    writable &&
    canRunStaged
  );
}

/**
 * The staged résumé pipeline's ENTRY POINT — the one surface that can honestly
 * start `resumePipeline.run`.
 *
 * `resume_pipeline_run` resolves both of its inputs SERVER-side: `resumeId`
 * through the `DocumentStore` and `jobId` through the live postings cache
 * (`match_resume::job_text_for`). Neither generate wizard can supply both — they
 * tailor free text that has no document id against a pasted ad that has no
 * posting id — which is why they always run the fast one-shot path instead.
 * This pane holds a full {@link Posting} (so `posting.id` really is a cached
 * posting) and the default saved résumé, i.e. both ids legitimately.
 *
 * The fast one-shot path has its own trigger next to this one in
 * `JobDetailPane` (the "Tailor" button); this panel exists for the staged
 * pipeline and the agentic "Improve this résumé" review that runs against its
 * output, so it has no depth choice of its own — every run here is a
 * `quality`-depth staged run.
 *
 * Three traps from the contract are wired here rather than assumed away:
 *
 * - **`job.completed` is not "done".** The draft streams under the run's own
 *   umbrella job, so the shared stream machinery completes that job while
 *   validation and up to two repair rounds are still ahead. Terminal state comes
 *   from {@link useResumePipelineSession}, which reads the run RECORD.
 * - **`stoppedReason` is nullable on a terminal run** — an absent one renders as
 *   no reason at all, never as "Finished" (`stoppedSuffix`); `status` is the
 *   authority, which is why the headline below is keyed on the machine state.
 * - **`needsReview` is not a finish.** It gets its own amber, count-carrying
 *   headline and never the completed copy.
 */
export function TailoredResumePanel({ posting }: { posting: Posting }) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const [reportOpen, setReportOpen] = useState(false);
  /** A run OTHER than this session's, opened from the history list. */
  const [otherRunId, setOtherRunId] = useState<string | null>(null);
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  const [stopRequested, setStopRequested] = useState(false);

  const resume = useDefaultResume();
  const { provider, model, effort } = useGenerateConfig();
  const providerReady = !!provider && !!model;
  /** The staged run needs the same two server-resolved inputs. */
  const canRunStaged = !!resume && providerReady;

  const session = useResumePipelineSession();
  const runs = usePipelineRunsForJob(posting.url).data ?? [];
  const regenerate = useRegenerateSection();
  const resolveFabrication = useResolveFabrication();

  // The report can be opened for this session's run or for an older one. Both
  // read the same query key, so the session's own record costs no extra fetch.
  const otherRun = usePipelineRun(otherRunId);
  const expandedRun = usePipelineRun(expandedRunId);
  const shownDetail = otherRunId ? (otherRun.data ?? null) : session.detail;
  const shownRunId = otherRunId ?? session.runId;

  const slot = shownDetail?.report?.resume ?? null;
  const contentReport = slot?.report ?? null;
  const documentText = shownDetail?.resumeText ?? '';
  const fabrications = useMemo(() => parseFabrications(slot?.fabrications), [slot]);
  const sections = useMemo(
    () => buildSectionVerdicts(contentReport, documentText),
    [contentReport, documentText]
  );
  const openClaims = unresolvedCount(fabrications, documentText);

  /**
   * Only the posting's NEWEST run may be written to — every run merges into one
   * saved résumé, so a write against an older one would edit a document that run
   * never produced. The backend refuses it either way; withholding the actions
   * means the user is not offered a button that is going to be rejected, and the
   * note below says why. A refusal that still happens (a newer run appeared
   * while this report was open) surfaces through `fixError`/`resolveError`.
   *
   * **This session's OWN run is writable without consulting the list**, and has
   * to be: `resume_pipeline_run` returns its ids before the `pipeline_runs` row
   * exists (the row is written inside the spawned task, after admission and
   * after five resolutions), so `start`'s invalidation routinely refetches a
   * list that still ends at the PREVIOUS run. Reading `runs[0]` alone then calls
   * the run the user is looking at "older than the newest", withholds Fix and
   * Resolve, and strands it at `needsReview` with no way to clear it. A session
   * that started a run cannot have been superseded without a newer run starting
   * — which would be this same session's, and it would know.
   */
  const newestRunId = runs[0]?.runId ?? null;
  const ownRun = !!session.runId && shownRunId === session.runId;
  const writable = !!shownRunId && (ownRun || !newestRunId || newestRunId === shownRunId);

  const busy = session.busy;
  const terminal = session.state !== 'idle' && !busy;

  const stage = session.stage;
  const stageLabel = stage
    ? t(`pipeline.stage.${stage.stage}`, { defaultValue: stage.stage })
    : t(`pipeline.state.${session.state}`, { defaultValue: '' });

  /**
   * The job ad's language, so a German posting is not answered in English.
   * `analyze_job` runs server-side and cannot be asked from here; the ad is the
   * only signal this surface has, and `unknown` (a snippet too short to detect)
   * falls back to the schema's own `'en'` rather than to a guess.
   */
  const targetLanguage = useMemo(() => {
    const detected = detectLanguage(posting.description ?? '');
    return detected === 'unknown' ? 'en' : detected;
  }, [posting.description]);

  /**
   * The only depth this panel ever sends. There is no picker any more (PR-4
   * dropped it) and no `max` to route to either — `quality` is what the fast
   * one-shot path next to this panel cannot do, which is the entire reason
   * this panel exists.
   */
  const handleStart = async () => {
    if (!resume || !providerReady) return;
    setStopRequested(false);
    await session.start({
      resumeId: resume.id,
      jobId: posting.id,
      jobUrl: posting.url,
      depth: 'quality',
      targetLanguage,
      // Budget and routing are backend-owned; the JD analysis is a STAGE of
      // the run, so there are no requirements to hand it and no letter in
      // scope.
      topRequirements: [],
      coverLetterText: '',
      // This panel is being replaced by TailorFlow's staged run (PR-3); it
      // never asks the pipeline to generate a letter itself.
      includeCoverLetter: false,
      ...(effort ? { effort } : {}),
    });
  };

  const stop = () => {
    setStopRequested(true);
    session.cancel();
  };

  const openReportFor = (runId: string) => {
    setOtherRunId(runId === session.runId ? null : runId);
    setReportOpen(true);
  };

  /**
   * The agentic review of the document this pane just produced — a second flow
   * behind the same `agent.run` command, started from here because this is the
   * only surface holding BOTH ids it needs (`resumeId` is the candidate's
   * MASTER résumé, the ground truth claims are checked against; the generation
   * under review is resolved server-side from `jobId`). The session lives at
   * this level, not inside the card it renders, so closing the modal mid-run
   * does not throw away a suspended confirm.
   */
  const improve = useAgentRunSession({
    kind: 'improve_resume',
    resumeId: resume?.id ?? null,
    jobId: posting.id,
    // A BODY sentence: `ImproveResumeRun` renders `failedTitle` as the alert's
    // heading and this string underneath it, so reusing the heading printed the
    // same sentence twice whenever the backend failed a run without a message.
    fallbackError: t('jobs.tailored.improve.failedFallback'),
  });

  /**
   * The report's own writes are withheld while a review is running, for a
   * different reason than the older-run rule — and the note has to say WHICH,
   * so it is the first branch: a live review already holds the document. It
   * captured the text at run start and is about to offer a corrected version
   * through `save_resume`, which replaces the aggregate wholesale; a Fix or a
   * Resolve landing in between is admitted by the limiter (the suspended run
   * holds one of two slots) and then silently overwritten on approve, with no
   * undo. Reading the report during a review stays available — only the writes
   * are the hazard.
   */
  const reportWritable = writable && !improve.busy;
  const note = [
    !otherRunId && resume
      ? t('jobs.tailored.report.source', {
          name: resume.name || t('jobs.tailored.unnamedResume'),
        })
      : null,
    improve.busy
      ? t('jobs.tailored.report.reviewInFlight')
      : writable
        ? t('jobs.tailored.report.removeHint')
        : t('jobs.tailored.report.olderRun'),
  ]
    .filter(Boolean)
    .join(' ');

  // Only a trail this surface actually FETCHED is handed over — an absent entry
  // makes the list render its "live progress only" note instead of an empty
  // ladder that would read as "this run had no stages".
  const eventsByRun = useMemo<Record<string, PipelineRunEvent[] | undefined>>(
    () => (expandedRunId && expandedRun.data ? { [expandedRunId]: expandedRun.data.events } : {}),
    [expandedRunId, expandedRun.data]
  );

  /**
   * Count CODE POINTS, matching the Rust clamp (`chars().take(RESUME_CAP)`);
   * `String.length` counts UTF-16 units and would over-report any text with an
   * astral character in it.
   */
  const reviewLength = useMemo(() => [...documentText].length, [documentText]);
  const tooLongToReview = reviewLength > GENERATION_REVIEW_CAP;
  /** Locale-formatted for the copy that quotes it — "8,000" / "8.000", never a
   *  bare machine number in a sentence. */
  const reviewCapLabel = useMemo(
    () => new Intl.NumberFormat(i18n.language).format(GENERATION_REVIEW_CAP),
    [i18n.language]
  );
  /** Each term is documented on {@link canImproveGeneration}. The jobless
   *  surfaces that also show a quality report (`TailorFlow`/`ai-generate`) get
   *  no entry at all: `agent.run` needs a real posting id and they have none. */
  const canImprove = canImproveGeneration({
    terminal,
    runState: session.state,
    hasDetail: !!shownDetail,
    hasDocument: !!documentText,
    writable,
    canRunStaged,
  });

  /** Where focus goes when the review card removes itself (see the card's
   *  `onDismissed`): the control that started it, which is still on screen. */
  const improveTriggerRef = useRef<HTMLButtonElement>(null);

  const titleId = 'tailored-resume-modal-title';
  const gateNoteId = 'tailored-resume-gate';
  const improveGateId = 'tailored-resume-improve-gate';
  /** The staged run cannot start — the note below says which half is missing,
   *  and Start points at it rather than being silently dead. */
  const gated = !busy && !canRunStaged;
  const showRuns = runs.length > 0;

  return (
    <>
      <Button
        variant="glass"
        onClick={() => setOpen(true)}
        title={t('jobs.tailored.triggerHint')}
        className="shrink-0 gap-1.5 text-brand-soft"
      >
        <ShieldCheck size={13} /> {t('jobs.tailored.trigger')}
        {/* A suspended review is invisible once this modal is closed, and the
            request expires in about five minutes — so the closed state has to
            carry the fact that one is waiting. Colour is not the only signal:
            the text is read out, and the button's own hint repeats it. */}
        {improve.pendingConfirm && (
          <span className="ml-0.5 inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-1.5 py-0.5 text-[9px] font-semibold text-amber-300">
            <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-amber-400" />
            {/* WORDS, not just the dot: a sighted user who is not running a
                screen reader would otherwise have colour as the only carrier
                (WCAG 1.4.1). The short label is what fits the trigger; the
                sr-only sentence adds the expiry. */}
            {t('jobs.tailored.improve.pendingBadgeShort')}
            <span className="sr-only">{t('jobs.tailored.improve.pendingBadge')}</span>
          </span>
        )}
      </Button>

      <ModalShell
        // One dialog at a time: `ModalShell`'s Escape handler is global and its
        // focus trap is per-instance, so the report REPLACES this shell rather
        // than stacking on it, and closing the report brings this one back.
        open={open && !reportOpen}
        onClose={() => setOpen(false)}
        maxWidth="max-w-lg"
        ariaLabelledby={titleId}
        header={
          <div className="flex items-start justify-between gap-3 border-b border-[var(--border-soft)] px-5 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <ShieldCheck size={14} className="shrink-0 text-brand-soft" />
                <span id={titleId} className="truncate text-sm font-semibold text-foreground/85">
                  {t('jobs.tailored.modalTitle')}
                </span>
              </div>
              <div className="mt-0.5 truncate text-[11px] text-foreground/40">
                {posting.title} · {posting.company}
              </div>
            </div>
            <Button
              onClick={() => setOpen(false)}
              aria-label={t('jobs.tailored.close')}
              className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
            >
              <X size={16} />
            </Button>
          </div>
        }
        footer={
          <div className="border-t border-[var(--border-soft)] px-5 py-3">
            {/* In the FOOTER, next to the control it explains. Below the
                scrollable body it was off-viewport from its own button, and the
                button could not speak for itself: a `disabled` control drops out
                of the tab order (so its `aria-describedby` is never announced)
                and this repo's Button kills pointer events on it (so `title`
                never fires either). Same guard as the reference below. */}
            {canImprove && tooLongToReview && (
              <p id={improveGateId} className="mb-2 text-[11px] leading-relaxed text-amber-400">
                {t('jobs.tailored.improve.tooLong', { max: reviewCapLabel })}
              </p>
            )}
            {/* Why Run again is dead and why the report has no Fix/Resolve
                right now. Visible adjacent copy, because a natively disabled
                Button in this repo can explain itself no other way. */}
            {improve.busy && (
              <p className="mb-2 text-[11px] leading-relaxed text-foreground/60">
                {t('jobs.tailored.improve.busyLock')}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2">
              {busy ? (
                <Button
                  variant="glass"
                  onClick={stop}
                  disabled={stopRequested}
                  title={t('jobs.tailored.cancelHint')}
                  className="w-full justify-center gap-1.5 text-foreground/60"
                >
                  <Square size={11} />{' '}
                  {stopRequested ? t('jobs.tailored.cancelling') : t('jobs.tailored.cancel')}
                </Button>
              ) : (
                <>
                  <Button
                    variant={terminal && shownDetail ? 'glass' : 'primary'}
                    // Locked while a review is in flight: that review is about
                    // to offer the CURRENT document back through its gated save,
                    // so a generation started underneath it would be silently
                    // overwritten by the older, reviewed text on approve.
                    disabled={gated || session.starting || improve.busy}
                    loading={session.starting}
                    onClick={() => void handleStart()}
                    // Same render guard as the note it points at — an
                    // `aria-describedby` to DOM that isn't there is a broken
                    // reference, not a hint.
                    {...(gated ? { 'aria-describedby': gateNoteId } : {})}
                    className="@sm:w-auto @sm:flex-1 w-full justify-center gap-1.5"
                  >
                    <ShieldCheck size={12} />{' '}
                    {terminal ? t('jobs.tailored.runAgain') : t('jobs.tailored.start')}
                  </Button>
                  {terminal && shownDetail && (
                    <Button
                      variant="primary"
                      onClick={() => setReportOpen(true)}
                      className="@sm:w-auto @sm:flex-1 w-full justify-center gap-1.5"
                    >
                      <FileText size={12} /> {t('jobs.tailored.openReport')}
                    </Button>
                  )}
                  {canImprove && (
                    <Button
                      ref={improveTriggerRef}
                      variant="glass"
                      // `aria-disabled`, not `disabled`, for the too-long
                      // refusal: a natively disabled button leaves the tab
                      // order, so the note it points at is never announced —
                      // the explanation would exist only for a sighted mouse
                      // user. The click is guarded instead. A review already in
                      // flight IS a native disable: its cause is on screen (the
                      // card below, with its own Stop).
                      disabled={improve.busy}
                      {...(tooLongToReview
                        ? { 'aria-disabled': true, 'aria-describedby': improveGateId }
                        : {})}
                      onClick={() => {
                        if (tooLongToReview) return;
                        void improve.start();
                      }}
                      // `aria-disabled` keeps pointer events, so unlike a
                      // natively disabled control this tooltip DOES fire — and
                      // a generic "what this does" hint on a button that will
                      // refuse the click is worse than none. Supplementary
                      // only: the footer note above is the primary explanation
                      // (a title is unreachable by keyboard and touch).
                      title={
                        tooLongToReview
                          ? t('jobs.tailored.improve.tooLongHint', { max: reviewCapLabel })
                          : t('jobs.tailored.improve.triggerHint')
                      }
                      className={cn(
                        '@sm:w-auto @sm:flex-1 w-full justify-center gap-1.5 text-brand-soft',
                        tooLongToReview && 'cursor-not-allowed opacity-45'
                      )}
                    >
                      <Sparkles size={12} aria-hidden="true" /> {t('jobs.tailored.improve.trigger')}
                    </Button>
                  )}
                </>
              )}
            </div>
          </div>
        }
      >
        <div className="space-y-4 px-5 py-4">
          {/* First in the body so the review's suspended confirm — the one thing
              that blocks the run — is visible without scrolling past the
              pipeline's own controls. */}
          {improve.active && (
            <ImproveResumeRun
              session={improve}
              onDismissed={() => improveTriggerRef.current?.focus()}
            />
          )}

          {/* Shown while no run is in flight — including AFTER a terminal one, so
              "run again" can be re-run without reopening. */}
          {!busy && (
            <>
              {session.state === 'idle' && (
                <div className="space-y-1.5 text-[11px] leading-relaxed text-foreground/50">
                  {resume && (
                    <p>
                      {t('jobs.tailored.usesResume', {
                        name: resume.name || t('jobs.tailored.unnamedResume'),
                      })}
                    </p>
                  )}
                  <p>{t('jobs.tailored.savesTo')}</p>
                </div>
              )}

              {gated && (
                <p
                  id={gateNoteId}
                  role="status"
                  className="text-[11px] leading-relaxed text-amber-400"
                >
                  {!resume ? t('jobs.tailored.needsResume') : t('jobs.tailored.needsProvider')}
                </p>
              )}
            </>
          )}

          {busy && (
            <div className="space-y-3">
              <div
                role="status"
                aria-busy="true"
                className="flex flex-col items-center text-center"
              >
                {/* Dots only when the run said how long it is: a reconnected run
                    replays a trail that carries no pipeline length, and an empty
                    dot row would claim a progress this surface doesn't know. */}
                {stage && stage.total > 0 ? (
                  <StepDots
                    currentStep={Math.min(stage.index, stage.total - 1)}
                    totalSteps={stage.total}
                    className="mb-2"
                  />
                ) : (
                  <Loader2
                    size={16}
                    className="mb-2 animate-spin text-brand-soft"
                    aria-hidden="true"
                  />
                )}
                <p className="text-[11px] font-medium text-brand-soft">{stageLabel}</p>
                {stage && stage.total > 0 && (
                  <p className="mt-0.5 text-[10px] uppercase tracking-[0.16em] text-foreground/35">
                    {t('pipeline.stageCounter', {
                      current: stage.index + 1,
                      total: stage.total,
                    })}
                  </p>
                )}
              </div>

              <p className="text-[10px] leading-relaxed text-foreground/40">
                {t('jobs.tailored.draftNote')}
              </p>
              {session.draft ? (
                <div className="max-h-52 select-text overflow-y-auto whitespace-pre-wrap rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
                  {session.draft}
                </div>
              ) : (
                <div className="space-y-2 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-3">
                  <Skeleton className="h-2.5 w-full" />
                  <Skeleton className="h-2.5 w-11/12" />
                  <Skeleton className="h-2.5 w-4/5" />
                </div>
              )}
            </div>
          )}

          {terminal && (
            <div className="space-y-3">
              <div className="flex flex-wrap items-center gap-2">
                <Tag
                  color={
                    session.state === 'done'
                      ? 'success'
                      : session.state === 'needsReview'
                        ? 'warning'
                        : session.state === 'error'
                          ? 'error'
                          : 'default'
                  }
                  className="text-[9px]"
                >
                  {t(`jobs.tailored.state.${session.state}`)}
                </Tag>
                {/* An absent reason contributes NOTHING — never "Finished". */}
                {shownDetail && stoppedSuffix(shownDetail.stoppedReason) && (
                  <span className="text-[10px] text-foreground/45">
                    {t(`pipeline.stopped.${stoppedSuffix(shownDetail.stoppedReason)}`)}
                  </span>
                )}
              </div>

              {session.state === 'needsReview' && (
                <div
                  role="status"
                  className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2.5"
                >
                  <p className="text-[11px] font-medium text-amber-400">
                    {t('jobs.tailored.needsReviewTitle', { count: openClaims })}
                  </p>
                  <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
                    {t('jobs.tailored.needsReviewHint')}
                  </p>
                </div>
              )}

              {session.state === 'done' && (
                <p className="text-[11px] leading-relaxed text-foreground/50">
                  {t('jobs.tailored.completedHint')}
                </p>
              )}

              {session.state === 'cancelled' && (
                <p className="text-[11px] leading-relaxed text-foreground/50">
                  {t('jobs.tailored.cancelledHint')}
                </p>
              )}

              {session.state === 'error' && (
                <div
                  role="alert"
                  className="rounded-lg border border-red-500/20 bg-red-500/5 px-3 py-2.5"
                >
                  <p className="text-[11px] font-medium text-red-400">
                    {t('jobs.tailored.failedTitle')}
                  </p>
                  {session.error && (
                    <p className="mt-1 text-[10px] leading-relaxed text-foreground/60">
                      {session.error}
                    </p>
                  )}
                  <p className="mt-1 text-[10px] leading-relaxed text-foreground/45">
                    {t('jobs.tailored.failedHint')}
                  </p>
                </div>
              )}

              {!!documentText && (
                <div>
                  <h3 className="mb-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
                    {t('jobs.tailored.document')}
                  </h3>
                  <div className="max-h-52 select-text overflow-y-auto whitespace-pre-wrap rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-[11px] leading-relaxed text-foreground/65">
                    {documentText}
                  </div>
                  <p className="mt-1 text-[10px] leading-relaxed text-foreground/35">
                    {t('jobs.tailored.savesTo')}
                  </p>
                </div>
              )}
            </div>
          )}

          {showRuns && (
            <PipelineRunsList
              runs={runs}
              onOpenReport={openReportFor}
              onExpand={setExpandedRunId}
              eventsByRun={eventsByRun}
              className="border-t border-white/[0.06] pt-4"
            />
          )}
        </div>
      </ModalShell>

      <QualityReportPanel
        open={reportOpen}
        onClose={() => {
          setReportOpen(false);
          setOtherRunId(null);
        }}
        report={contentReport}
        docKind="resume"
        pipeline={{
          documentText,
          sections,
          fabrications,
          note,
          // Withheld for an older run — see `writable`. `onRemoveEvidence` is
          // withheld ALWAYS: this surface has no editor for the saved résumé, so
          // a Remove records the verdict and the row honestly reports the line
          // as still present rather than claiming a deletion that never happened.
          // The run row agrees: `report::unresolved_count` applies the same
          // document-agreement rule as `isFabricationResolved`, so the run
          // stays `needsReview` — headline and review row cannot disagree —
          // until the user edits the line out where the document lives (the
          // saved application, per the `savesTo` hint).
          ...(reportWritable && shownRunId
            ? {
                onFixSection: (sectionKey: PipelineSectionKey, noteText: string) =>
                  regenerate.mutate({
                    runId: shownRunId,
                    sectionKey,
                    ...(noteText ? { note: noteText } : {}),
                  }),
                onResolveFabrication: (issueKey: string, decision: 'remove' | 'keep') =>
                  resolveFabrication.mutate({ runId: shownRunId, issueKey, decision }),
              }
            : {}),
          fixingSection: regenerate.isPending ? (regenerate.variables?.sectionKey ?? null) : null,
          fixError: regenerate.error
            ? t('jobs.tailored.fixFailed', { detail: errorDetail(regenerate.error) })
            : null,
          resolvingIssueKey: resolveFabrication.isPending
            ? (resolveFabrication.variables?.issueKey ?? null)
            : null,
          resolveError: resolveFabrication.error
            ? t('jobs.tailored.resolveFailed', { detail: errorDetail(resolveFabrication.error) })
            : null,
          ...(shownDetail?.metrics.repairRounds != null
            ? { repairRounds: shownDetail.metrics.repairRounds }
            : {}),
          ...(shownDetail?.metrics.reverted != null
            ? { repairReverted: shownDetail.metrics.reverted }
            : {}),
        }}
      />
    </>
  );
}
