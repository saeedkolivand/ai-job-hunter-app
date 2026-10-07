import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { AiGenerationRecord } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useQualityRecheck } from '@/hooks/use-quality-recheck';
import {
  type ResumePipelineSession,
  useResumePipelineSession,
} from '@/hooks/use-resume-pipeline-session';
import {
  countryFromLocation,
  type GenerationMeta,
  type LetterLayoutId,
  parseQualityReport,
  type QualityReport,
  resolveMarket,
  type TemplateId,
} from '@/lib/generate';
import { RESUME_PIPELINE_BUSY_STATES } from '@/lib/machines/resume-pipeline.machine';
import { keys } from '@/services/query-client';
import { useGenerateConfig } from '@/services/use-ai-provider';
import { usePipelineRunsForJob } from '@/services/use-resume-pipeline';

import { pipelineStepForStage } from './lib/pipeline-steps';
import type { TailorWizardState } from './lib/tailor-state';
import type { TailorTarget } from './lib/tailor-target';
import { resolveTargetLanguage } from './useTailorPipeline/resolveTargetLanguage';
import { useEditPersistence } from './useTailorPipeline/useEditPersistence';
import { usePipelineReview } from './useTailorPipeline/usePipelineReview';
import { useTailorOutputActions } from './useTailorPipeline/useTailorOutputActions';

export { resolveTargetLanguage };

interface Params {
  jobDesc: string;
  /** The résumé the wizard is tailoring FROM — context for the quality panel's
   *  "Re-check" (the run itself takes it per call via `start`). */
  sourceResume: string;
  /** The found job's URL — empty for an unlinked (pasted/no-URL) posting. Sent
   *  through honestly; never fabricated (see the plan's gotcha on this). */
  jobUrl: string;
  jobTitle: string;
  companyName: string;
  /** Free-text job location as written on the ad (e.g. "New York, NY, US",
   *  "Köln, Deutschland") — feeds {@link countryFromLocation} for the export
   *  market. Empty/absent when the posting doesn't state one. */
  jobLocation?: string;
  /** The board the job came from (e.g. "linkedin"), text-path posting identity. */
  board: string;
  canUse: boolean;
  hasDesc: boolean;
  /**
   * Which document(s) the run on screen produces — the host's `generatedTarget`
   * (the PERSISTED wizard form, falling back to the live one). Read here for
   * one reason: it decides which document the results panel opens on.
   *
   * A prop rather than something `start()` remembers, because the cold path has
   * no `start()` to remember anything: navigating away and back, or reopening
   * the app on a posting whose last run was cover-only, remounts this hook with
   * a finished run and no session. The persisted form is the only surviving
   * answer to "what did that run produce", so the derivation has to hang off it.
   */
  target: TailorTarget;
  templateId: TemplateId;
  atsMode: boolean;
  accent?: string;
  letterLayoutId?: LetterLayoutId;
  /**
   * This job's aggregate `ai_generations` record, if any — the staged pipeline
   * writes résumé AND letter text onto it directly, so this is where the
   * LETTER text comes from (the résumé instead comes from the run detail,
   * `PipelineRunDetail.resumeText`, which the run's own `get` keeps current).
   * Recomputed live by the host every render (same prop TailorFlow already
   * threads through as `seedGeneration`) — never cached here.
   */
  latestGeneration?: AiGenerationRecord;
  /** Reconnect target — the `{runId, jobId}` persisted for this contextId. */
  initialRunId?: string | null;
  initialJobId?: string | null;
  /** Called once a run starts (or reconnects) so the host can persist the ids
   *  for next time (survives navigation away and back). */
  onRunStarted?: (ids: { runId: string; jobId: string }) => void;
}

/**
 * Runs the staged quality pipeline for the tailor flow — the replacement for
 * `useTailorGeneration`'s one-shot path (PR-3 of the staged-cutover plan).
 * Thin adapter over {@link useResumePipelineSession}: this hook owns only
 * page-local UI state (active doc, copy/export, inline-edit overrides) plus
 * the translation/formatting glue the session doesn't know about.
 */
export function useTailorPipeline({
  jobDesc,
  sourceResume,
  jobUrl,
  jobTitle,
  companyName,
  jobLocation,
  board,
  canUse,
  hasDesc,
  target,
  templateId,
  atsMode,
  accent,
  letterLayoutId,
  latestGeneration,
  initialRunId,
  initialJobId,
  onRunStarted,
}: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const qc = useQueryClient();
  const runs = usePipelineRunsForJob(jobUrl).data ?? [];

  const session = useResumePipelineSession(initialRunId, initialJobId);
  // Same per-provider effort the other generation surfaces send (`stream.ts`).
  const { effort } = useGenerateConfig();

  // `onRunStarted` is host-supplied and, on the real DocumentsTab/TailorFlow
  // wiring, a FRESH arrow every render (and calling it writes a Zustand
  // slice, which always returns a new object → re-renders the host → a new
  // arrow again). Reading it from a ref keeps the effect below from ever
  // listing it as a dependency, so a host re-render alone can't re-fire it —
  // only an actual new `{runId, jobId}` can. `persistedRunRef` is a second,
  // independent guard: even if this effect DOES re-run for the same run
  // (e.g. a remount that re-seeds identical ids), it's a no-op rather than a
  // redundant host write. Together these close the infinite update loop
  // (`onRunStarted` → host state → new arrow → effect refires → …).
  const onRunStartedRef = useRef(onRunStarted);
  onRunStartedRef.current = onRunStarted;
  const persistedRunRef = useRef<string | null>(null);
  useEffect(() => {
    if (!session.runId || !session.jobId) return;
    const key = `${session.runId}|${session.jobId}`;
    if (persistedRunRef.current === key) return;
    persistedRunRef.current = key;
    onRunStartedRef.current?.({ runId: session.runId, jobId: session.jobId });
  }, [session.runId, session.jobId]);

  // The 4-step checklist position — keeps the LAST known step for a stage
  // name this build doesn't map (see `pipelineStepForStage`), never regresses.
  const [currentStep, setCurrentStep] = useState(0);
  const stageName = session.stage?.stage;
  useEffect(() => {
    if (!stageName) return;
    setCurrentStep((prev) => pipelineStepForStage(stageName, prev));
  }, [stageName]);

  // A stage name this build doesn't have a `pipeline.stage.*` copy entry for
  // (added server-side after this renderer shipped) falls back to the
  // machine's own translated coarse state — never the raw snake_case wire
  // name, which would leak straight onto the panel.
  const stageLabel = session.stage
    ? t(`pipeline.stage.${session.stage.stage}`, {
        defaultValue: t(`pipeline.state.${session.state}`, { defaultValue: '' }),
      })
    : t(`pipeline.state.${session.state}`, { defaultValue: '' });

  // Modal-local, ephemeral UI — fine to reset on remount.
  //
  // DERIVED from the run's target, with the user's own tab click as the
  // override. It used to be a plain `useState('resume')` that nothing ever
  // corrected, so a cover-only run opened on the résumé tab and showed the
  // résumé — which, together with `GenerationOutput`'s tab list being built
  // from this value, is the whole reason "Cover letter" looked like it had
  // generated a résumé. Deriving rather than seeding in `start()` is what also
  // covers the COLD path: a remount (navigate away and back, or a fresh app
  // start on a posting whose last run was cover-only) has a finished run and no
  // `start()` call to have seeded anything.
  const [activeOutOverride, setActiveOut] = useState<'resume' | 'cover' | null>(null);
  const activeOut: 'resume' | 'cover' =
    activeOutOverride ?? (target === 'cover' ? 'cover' : 'resume');

  // Local overrides for a hand-edit — the run record / aggregate are the
  // source of truth until the user types, exactly like the fast path's
  // session-store outputs were.
  const [resumeOverride, setResumeOverride] = useState<string | null>(null);
  const [letterOverride, setLetterOverride] = useState<string | null>(null);

  const persistEdit = useEditPersistence();

  // `session.detail` is the LIVE run's own document — present once this
  // session started or reconnected to a run. `latestGeneration` (the job's
  // aggregate, threaded through from a live query one level up) is what
  // fills a COLD entry instead: no `runId` was ever persisted for this
  // session (a fresh app start, or a different surface produced the run),
  // but the posting already has a saved result. Same fallback shape both
  // texts already need for the letter (which has no run-detail source at
  // all) — the résumé just has one more rung.
  const resumeOut =
    resumeOverride ?? session.detail?.resumeText ?? latestGeneration?.resumeText ?? '';
  const coverOut = letterOverride ?? latestGeneration?.coverLetterText ?? '';
  const output = activeOut === 'resume' ? resumeOut : coverOut;
  const hasOutput = !!(resumeOut || coverOut);

  // Structurally identical to the renderer's own `QualityReport` (see
  // `PipelineQualityReport`'s doc comment) — every slot the fast path's parser
  // produces is present here too, plus the pipeline-only `fabrications`, which
  // `QualityReportSlot` already documents as opaque additional data. No cast
  // needed: `PipelineQualityReportSlot` is a strict superset. Prefers the live
  // run's own report, but falls back to the aggregate's persisted wrapper
  // (parsed the same way the fast path always has) whenever the live one is
  // absent — both for a cold entry (no `session.detail` at all) AND for a run
  // that ended before the `validate` stage ever ran (cancel, or a deadline
  // stop at a stage boundary), which leaves `session.detail.report === null`.
  // Without the fallback either case blanks the whole quality panel even
  // though the aggregate still holds a perfectly good report from an earlier
  // run.
  const report: QualityReport | null =
    session.detail?.report ?? parseQualityReport(latestGeneration?.qualityReport);

  // Regenerate must keep writing in whatever language the last run actually
  // produced, not re-detect from `jobDesc` and collapse to English the moment
  // detection can't tell (empty/short text, an unmapped ISO code, franc
  // returning `und`) — `latestGeneration` already carries the answer, WHEN it
  // is a confident one (see {@link resolveTargetLanguage}'s doc comment for
  // the owner decision this chain implements: persist/prefer only a confident
  // value, never a guess).
  const { language: targetLanguage, confident: targetLanguageConfident } = useMemo(
    () =>
      resolveTargetLanguage(
        {
          targetLanguage: latestGeneration?.targetLanguage,
          jobAdLanguage: latestGeneration?.jobAdLanguage,
        },
        jobDesc
      ),
    [jobDesc, latestGeneration?.targetLanguage, latestGeneration?.jobAdLanguage]
  );

  // Export/preview market — derived from the found job's free-text `location`
  // (unlike `AIGeneratePage`'s extracted meta, this hook never had a structured
  // `jobCountry`), falling back to the letter-language default when the
  // location doesn't resolve to a known country (see `resolveMarket`). Still
  // far better than the `undefined` this hook used to send, which the Rust
  // exporter silently treats as "intl" and skips market-specific conventions
  // (e.g. DIN 5008 for `de`, US Letter for `us`) entirely.
  const market = useMemo(
    () =>
      resolveMarket({
        jobCountry: countryFromLocation(jobLocation, targetLanguage),
        targetLanguage,
      }),
    [jobLocation, targetLanguage]
  );

  // A best-effort stand-in for the fast path's model-extracted `meta`: the
  // staged run resolves job title/company server-side (or from the request,
  // on the text path) but never echoes a structured meta object back over
  // IPC. Seeded from `latestGeneration` (the job's persisted aggregate, when
  // one exists) rather than fabricated — a blank `topRequirements` silently
  // strips the "top requirement hits" metric out of a Re-check
  // (`use-quality-recheck.ts`), and a non-null-but-empty `meta` short-circuits
  // the answers assistant's own metadata extraction
  // (`useApplicationAnswers.ts` treats any non-null `meta` as already
  // detected). `jobTitle`/`companyName` stay off the live props, not the
  // record — this hook already gets those fresh off the current posting every
  // render, which a persisted record can lag. `candidateName` only feeds the
  // export FILENAME's cosmetic fallback ("Candidate-…") — ADR-0021 keeps the
  // editor as the header's authority at export time, unaffected by this.
  const meta: GenerationMeta | null = hasOutput
    ? {
        candidateName: latestGeneration?.candidateName || '',
        jobTitle,
        companyName,
        resumeLanguage: latestGeneration?.resumeLanguage || targetLanguage,
        jobAdLanguage: latestGeneration?.jobAdLanguage || targetLanguage,
        mismatch: latestGeneration?.mismatch ?? false,
        targetLanguage,
        topRequirements: latestGeneration?.topRequirements ?? [],
      }
    : null;

  // Refresh the aggregate (letter text, quality-report fallback context) and
  // the autopilot score once the SESSION reaches a terminal state — nothing
  // else invalidates either, and the app's global refetch-on-focus/mount is
  // off. Keyed on `session.state`, not `session.detail?.status`: a run that
  // dies via the umbrella `job.failed` path (a full queue, no configured
  // provider, a deleted résumé, …) sends `ERROR` straight to the machine with
  // NO run record ever written (see `use-resume-pipeline-session.ts`'s
  // `handleJobEvent` doc) — `detail` stays `null` forever, so gating on its
  // `status` field would leave the panel showing stale pre-run data
  // indefinitely. "Terminal" here is "not idle and not busy" rather than an
  // enumerated list, so a future terminal state added to the machine is
  // covered without an edit here. `pipelineState` as the sole dependency
  // mirrors this hook's other re-render-loop guards (`onRunStartedRef`,
  // `persistedRunRef`): a re-render while already terminal is a no-op, only
  // an actual state change re-fires this.
  const pipelineState = session.state;
  useEffect(() => {
    if (pipelineState === 'idle' || RESUME_PIPELINE_BUSY_STATES.includes(pipelineState)) return;
    void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
  }, [qc, pipelineState]);

  const onReportChange = useCallback(
    (next: QualityReport) => {
      if (!session.runId) return;
      qc.setQueryData<PipelineRunDetail | null>(keys.pipeline.run(session.runId), (old) =>
        old ? { ...old, report: next } : old
      );
    },
    [qc, session.runId]
  );

  /**
   * Whether the document currently on screen is one THIS run produced.
   *
   * Only a cover-letter-only run can answer `false`, and only on its résumé
   * tab: that tab shows the posting's older tailored résumé, which stays
   * viewable and exportable (see `GenerationOutput`'s tab list) but must not be
   * acted on by this run's machinery.
   *
   * Two affordances read it, and both WRITE:
   *
   * * **Fix section** (`pipelineReview`). `session.detail.report` comes off the
   *   per-job AGGREGATE, not the run (`resume_pipeline_get` joins
   *   `find_for_job`), so the older `resume` slot is still present here and the
   *   panel would render Fix buttons over it. `regenerateSection` would ACCEPT
   *   the click — `ensure_latest_run` only asks whether this is the posting's
   *   newest run, which it is — and spend a provider call rewriting a document
   *   this run never wrote. (Also refused server-side now; this is the half
   *   that stops the button existing.)
   * * **Re-check** (`recheck`). It re-validates the ACTIVE document and
   *   `persistReport`s the merged wrapper back onto the aggregate, so it would
   *   overwrite the posting's report with one computed under a run that has no
   *   résumé of its own.
   *
   * Gated by withholding `onReportChange` rather than by a second condition on
   * `recheck`: `useQualityRecheck` already returns `recheck: undefined` when it
   * has no session writer ("no way to show a result — hide the action"), so
   * this reuses that rule instead of adding a parallel one that could drift
   * from it.
   *
   * Deliberately NOT gated: inline editing (`editActiveOutput`) and export.
   * Those are ordinary hand edits and downloads of a document the user already
   * has saved — the same thing the Documents editor does — and they are the
   * reason the tab is shown at all. Audited against every other `activeOut`
   * reader in this hook; `output` and the export `docType` are read-only.
   */
  const activeIsThisRunsOwn = !(target === 'cover' && activeOut === 'resume');

  const { recheck, rechecking } = useQualityRecheck({
    report,
    meta,
    sourceResume,
    jobAd: jobDesc,
    docKind: activeOut === 'resume' ? 'resume' : 'coverLetter',
    onReportChange: activeIsThisRunsOwn ? onReportChange : undefined,
    resumeText: resumeOut,
    coverLetterText: coverOut,
    generating: session.busy,
    jobUrl,
    board,
  });

  const { openClaimsTotal, pipelineReview } = usePipelineReview({
    session,
    activeOut,
    activeIsThisRunsOwn,
    output,
    resumeOut,
    coverOut,
  });

  const editActiveOutput = (text: string) => {
    if (activeOut === 'resume') setResumeOverride(text);
    else setLetterOverride(text);
    const id = latestGeneration?.id;
    if (!id) return;
    persistEdit(activeOut === 'resume' ? 'resume' : 'cover', id, text);
  };

  const start = async (values: TailorWizardState) => {
    if (!canUse || !hasDesc) return null;
    setResumeOverride(null);
    setLetterOverride(null);
    // A regenerate that CHANGES the target must not strand the panel on a tab
    // the new run does not produce — drop back to the derived default.
    setActiveOut(null);
    setCurrentStep(0);
    const resumeId = values.resumeDocId ?? '';
    // Computed HERE, not in a memo — a memo evaluated at mount would go stale
    // in a session left open across midnight, and a wrong date on a cover
    // letter is worse than none. `targetLanguage` is a valid BCP-47 tag from
    // `detectLanguage`, but `toLocaleDateString` still throws `RangeError` on
    // a malformed one — fall back to the runtime default locale rather than
    // failing the whole run over a date string.
    let today: string;
    try {
      today = new Date().toLocaleDateString(targetLanguage, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
      });
    } catch {
      today = new Date().toLocaleDateString(undefined, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
      });
    }
    // What goes on the WIRE deliberately differs from `targetLanguage` above
    // when the resolution wasn't confident (the tier-4 'en' guess): Rust
    // writes whatever `targetLanguage` it receives straight onto the
    // `ai_generations` aggregate, UNCONDITIONALLY, at
    // `commands/resume_pipeline/mod.rs:712` — there is no confidence concept
    // on that write, and it cannot be added from here (Rust is out of scope
    // for this change; see the handoff note in the PR). Sending '' instead of
    // the guess needs no schema change: `ResumePipelineRunSchema.targetLanguage`
    // (`packages/shared/src/schemas/index.ts`) only `.default('en')`s an
    // ABSENT key, so an explicit '' passes through untouched;
    // `normalize_language('')` already treats it as 'en' for every
    // prompt/validation use; and `ai_generations::merge_application`'s `pick` (`ai_generations/record.rs`)
    // already keeps whatever the record had for an empty INCOMING field — the
    // exact mechanism that already protects `resumeLanguage`/`jobAdLanguage`
    // from a stale overwrite. A guess therefore still runs THIS generation
    // (via the local `targetLanguage` above) but is never remembered, so it
    // can never win `resolveTargetLanguage`'s tier-1 "persisted confident
    // value" branch on a later run. `AiGenerateRequest.locale` (the draft/
    // cover-letter stages' own use of this same value) is set from the raw
    // target_language and is unread by every provider adapter today — an
    // empty value there is a no-op, not a behavior change (verified by
    // reading, not editing, `commands/ai_provider/**`).
    const wireTargetLanguage = targetLanguageConfident ? targetLanguage : '';
    const runId = await session.start({
      resumeId,
      resumeText: resumeId ? '' : values.resume,
      jobId: '',
      jobAdText: jobDesc,
      jobTitle,
      companyName,
      board,
      jobUrl,
      targetLanguage: wireTargetLanguage,
      // Same value the export/preview path already resolved via this hook's
      // `market` memo — sent through unchanged so the letter prompt and the
      // export layout agree on one market.
      market,
      today,
      topRequirements: [],
      coverLetterText: '',
      // TWO independent flags, not one three-valued token. `'cover'` used to
      // collapse onto the byte-identical request `'both'` sends, because there
      // was no résumé flag to turn off — so the backend ran, validated,
      // repaired and PERSISTED a résumé for a run that asked for a letter.
      includeResume: values.outputType !== 'cover',
      includeCoverLetter: values.outputType !== 'resume',
      researchCompany: values.researchCompany,
      effort,
    });
    // `session.start` already logged the cause and set `error`/`state` — this
    // is the one thing it can't do itself: a transient, dismissable toast.
    // The persistent banner (rendered off the same `error`) is the durable
    // half of that pair.
    if (!runId) notify.error({ message: t('autopilot.apply.failed') });
    return runId;
  };

  const cancel = () => session.cancel();

  const { copied, exportOpen, setExportOpen, copy, exportAs } = useTailorOutputActions({
    output,
    activeOut,
    meta,
    market,
    templateId,
    atsMode,
    accent,
    letterLayoutId,
  });

  return {
    state: session.state,
    busy: session.busy,
    starting: session.starting,
    currentStep,
    stageLabel,
    // The run's own backend-recorded start time (`pipeline_runs.started_at`),
    // for `GeneratingPanel`'s elapsed caption — anchoring on this instead of
    // the panel's own mount time is what survives a navigate-away-and-back
    // (see that prop's doc comment for why a remount needs it). `> 0`, not a
    // bare presence check: a `0`/negative epoch ms is never a real timestamp
    // (defensive — today's `now_ms()`-populated column never produces one),
    // and falling through to `null` here is what lets `GeneratingPanel`'s own
    // `runStartedAt ?? mountFallback` recover instead of anchoring the
    // caption on the Unix epoch and counting up from ~1970.
    runStartedAt:
      session.detail?.startedAt && session.detail.startedAt > 0 ? session.detail.startedAt : null,
    thinking: session.thinking,
    // The résumé pane's live stream — the letter's own (`session.letterDraft`)
    // is display-only in the same sense, exposed separately so a cover-only
    // run's checklist doesn't show résumé tokens under the "Generate" step.
    draft: session.draft,
    letterDraft: session.letterDraft,
    resumeOut,
    coverOut,
    activeOut,
    setActiveOut,
    output,
    hasOutput,
    error: session.error,
    stoppedReason: session.detail?.stoppedReason,
    copied,
    exportOpen,
    setExportOpen,
    start,
    cancel,
    copy,
    exportAs,
    editActiveOutput,
    meta,
    // Whether `meta`'s three language fields are a confident detection or the
    // tier-4 English guess (see `resolveTargetLanguage`'s doc comment). `meta`
    // itself keeps carrying the guess unconditionally — every current-run
    // consumer (live preview, rewrite locale, filename, job-ad summary) needs
    // a usable value NOW, exactly like `start()`'s own local `targetLanguage`.
    // Only a SAVE path (`useApplicationAnswers`, `useInterviewQuestions`) may
    // use this flag to withhold the guess from the wire, mirroring
    // `wireTargetLanguage` below.
    targetLanguageConfident,
    // Export/preview market — see the computation's doc comment above. The live
    // preview (GenerationOutput → PdfPreview) needs the SAME value the export
    // sends, or the on-screen letter's salutation/sign-off silently disagrees
    // with the downloaded one (the Rust exporter defaults to "intl" on `undefined`).
    market,
    report,
    pipelineReview,
    openClaimsTotal,
    recheck,
    rechecking,
    runs,
  };
}

export type TailorPipelineSession = ReturnType<typeof useTailorPipeline>;
export type { ResumePipelineSession };
