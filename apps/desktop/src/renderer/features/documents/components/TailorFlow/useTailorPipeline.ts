import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { type AiGenerationRecord, detectLanguage } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import type { QualityPipelineReview } from '@/components/generation/QualityReportPanel';
import { useQualityRecheck } from '@/hooks/use-quality-recheck';
import {
  type ResumePipelineSession,
  useResumePipelineSession,
} from '@/hooks/use-resume-pipeline-session';
import { errorClass, errorDetail } from '@/lib/error-class';
import {
  buildFilename,
  buildSectionVerdicts,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMeta,
  type LetterLayoutId,
  parseFabrications,
  parseQualityReport,
  PERSIST_DEBOUNCE_MS,
  type QualityReport,
  type TemplateId,
  unresolvedCount,
} from '@/lib/generate';
import { COPY_FEEDBACK_MS } from '@/lib/timings';
import { keys } from '@/services/query-client';
import { useUpdateAiGeneration } from '@/services/use-ai-generations';
import {
  usePipelineRunsForJob,
  useRegenerateSection,
  useResolveFabrication,
} from '@/services/use-resume-pipeline';

import { pipelineStepForStage } from './lib/pipeline-steps';
import type { TailorWizardState } from './lib/tailor-state';

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
  /** The board the job came from (e.g. "linkedin"), text-path posting identity. */
  board: string;
  canUse: boolean;
  hasDesc: boolean;
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
  board,
  canUse,
  hasDesc,
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
  const updateAiGeneration = useUpdateAiGeneration();
  const regenerate = useRegenerateSection();
  const resolveFabrication = useResolveFabrication();
  const runs = usePipelineRunsForJob(jobUrl).data ?? [];

  const session = useResumePipelineSession(initialRunId, initialJobId);

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
  const [activeOut, setActiveOut] = useState<'resume' | 'cover'>('resume');
  const [copied, setCopied] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);

  // Local overrides for a hand-edit — the run record / aggregate are the
  // source of truth until the user types, exactly like the fast path's
  // session-store outputs were.
  const [resumeOverride, setResumeOverride] = useState<string | null>(null);
  const [letterOverride, setLetterOverride] = useState<string | null>(null);

  const persistTimers = useRef<{
    resume?: ReturnType<typeof setTimeout>;
    cover?: ReturnType<typeof setTimeout>;
  }>({});
  useEffect(() => {
    const timers = persistTimers.current;
    return () => {
      if (timers.resume) clearTimeout(timers.resume);
      if (timers.cover) clearTimeout(timers.cover);
    };
  }, []);

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
  // needed: `PipelineQualityReportSlot` is a strict superset. Cold-entry falls
  // back to the aggregate's OWN persisted wrapper, parsed the same way the
  // fast path always has — never both at once, so a live run's own report
  // can't be shadowed by a stale persisted one.
  const report: QualityReport | null = session.detail
    ? session.detail.report
    : latestGeneration
      ? parseQualityReport(latestGeneration.qualityReport)
      : null;

  const targetLanguage = useMemo(() => {
    const detected = detectLanguage(jobDesc);
    return detected === 'unknown' ? 'en' : detected;
  }, [jobDesc]);

  // A best-effort stand-in for the fast path's model-extracted `meta`: the
  // staged run resolves job title/company server-side (or from the request,
  // on the text path) but never echoes a structured meta object back over
  // IPC. `candidateName` stays blank — ADR-0021 makes the editor the header's
  // authority at export time, so this only costs the filename's cosmetic
  // fallback ("Candidate-…"), never the document itself.
  const meta: GenerationMeta | null = hasOutput
    ? {
        candidateName: '',
        jobTitle,
        companyName,
        resumeLanguage: targetLanguage,
        jobAdLanguage: targetLanguage,
        mismatch: false,
        targetLanguage,
        topRequirements: [],
      }
    : null;

  // Refresh the aggregate (letter text, quality-report fallback context) once
  // the run leaves `running` — nothing else invalidates it, and the app's
  // global refetch-on-focus/mount is off.
  const status = session.detail?.status;
  useEffect(() => {
    if (!status || status === 'running') return;
    void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
  }, [qc, status]);

  const onReportChange = useCallback(
    (next: QualityReport) => {
      if (!session.runId) return;
      qc.setQueryData<PipelineRunDetail | null>(keys.pipeline.run(session.runId), (old) =>
        old ? { ...old, report: next } : old
      );
    },
    [qc, session.runId]
  );

  const { recheck, rechecking } = useQualityRecheck({
    report,
    meta,
    sourceResume,
    jobAd: jobDesc,
    docKind: activeOut === 'resume' ? 'resume' : 'coverLetter',
    onReportChange,
    resumeText: resumeOut,
    coverLetterText: coverOut,
    generating: session.busy,
    jobUrl,
    board,
  });

  // Section-fix / fabrication-review extras for the ACTIVE document — this
  // session's OWN run is always the posting's newest (nothing else can start
  // one from here), so unlike `TailoredResumePanel` there is no older-run
  // gate to apply.
  // Read off the RAW `PipelineQualityReport` (not the renderer-shaped `report`
  // above) — its slot type declares `fabrications`, where `QualityReportSlot`
  // deliberately doesn't (it's opaque additional data there).
  const rawSlot =
    activeOut === 'resume' ? session.detail?.report?.resume : session.detail?.report?.coverLetter;
  const sections = useMemo(() => buildSectionVerdicts(rawSlot?.report, output), [rawSlot, output]);
  const fabrications = useMemo(() => parseFabrications(rawSlot?.fabrications), [rawSlot]);
  const runId = session.detail?.runId;

  // Unresolved fabrication count across BOTH documents — the Rust
  // `needsReview` verdict (`still_needs_review`) scans resume AND coverLetter,
  // but `fabrications` above (and the ACTIVE-tab-only `pipelineReview` it
  // feeds) only ever reflects whichever document is on screen. A run flagged
  // for review while the user is looking at the OTHER, clean document must
  // not read as "0 claims" just because this session hasn't switched tabs.
  const resumeReportSlot = session.detail?.report?.resume;
  const coverReportSlot = session.detail?.report?.coverLetter;
  const openClaimsTotal = useMemo(() => {
    const resumeUnresolved = resumeReportSlot
      ? unresolvedCount(parseFabrications(resumeReportSlot.fabrications), resumeOut)
      : 0;
    const coverUnresolved = coverReportSlot
      ? unresolvedCount(parseFabrications(coverReportSlot.fabrications), coverOut)
      : 0;
    return resumeUnresolved + coverUnresolved;
  }, [resumeReportSlot, coverReportSlot, resumeOut, coverOut]);
  const pipelineReview: QualityPipelineReview | undefined = runId
    ? {
        documentText: output,
        sections,
        fabrications,
        onFixSection: (sectionKey, note) =>
          regenerate.mutate({ runId, sectionKey, ...(note ? { note } : {}) }),
        fixingSection: regenerate.isPending ? (regenerate.variables?.sectionKey ?? null) : null,
        fixError: regenerate.error
          ? t('autopilot.apply.wizard.results.fixFailed', { detail: errorDetail(regenerate.error) })
          : null,
        onResolveFabrication: (issueKey, decision) =>
          resolveFabrication.mutate({ runId, issueKey, decision }),
        resolvingIssueKey: resolveFabrication.isPending
          ? (resolveFabrication.variables?.issueKey ?? null)
          : null,
        resolveError: resolveFabrication.error
          ? t('autopilot.apply.wizard.results.resolveFailed', {
              detail: errorDetail(resolveFabrication.error),
            })
          : null,
        ...(session.detail?.metrics.repairRounds != null
          ? { repairRounds: session.detail.metrics.repairRounds }
          : {}),
        ...(session.detail?.metrics.reverted != null
          ? { repairReverted: session.detail.metrics.reverted }
          : {}),
      }
    : undefined;

  const editActiveOutput = (text: string) => {
    if (activeOut === 'resume') setResumeOverride(text);
    else setLetterOverride(text);
    const id = latestGeneration?.id;
    if (!id) return;
    const field = activeOut === 'resume' ? 'resume' : 'cover';
    const existing = persistTimers.current[field];
    if (existing) clearTimeout(existing);
    persistTimers.current[field] = setTimeout(() => {
      updateAiGeneration.mutate(
        activeOut === 'resume' ? { id, resumeText: text } : { id, coverLetterText: text }
      );
    }, PERSIST_DEBOUNCE_MS);
  };

  const start = async (values: TailorWizardState) => {
    if (!canUse || !hasDesc) return null;
    setResumeOverride(null);
    setLetterOverride(null);
    setCurrentStep(0);
    const resumeId = values.resumeDocId ?? '';
    const runId = await session.start({
      resumeId,
      resumeText: resumeId ? '' : values.resume,
      jobId: '',
      jobAdText: jobDesc,
      jobTitle,
      companyName,
      board,
      jobUrl,
      targetLanguage,
      topRequirements: [],
      coverLetterText: '',
      includeCoverLetter: values.outputType !== 'resume',
    });
    // `session.start` already logged the cause and set `error`/`state` — this
    // is the one thing it can't do itself: a transient, dismissable toast.
    // The persistent banner (rendered off the same `error`) is the durable
    // half of that pair.
    if (!runId) notify.error({ message: t('autopilot.apply.failed') });
    return runId;
  };

  const cancel = () => session.cancel();

  const copy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
  };

  const exportAs = async (fmt: 'pdf' | 'docx' | 'txt') => {
    setExportOpen(false);
    if (!output) return;
    const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const fileMeta: GenerationMeta = meta ?? {
      candidateName: '',
      jobTitle: '',
      companyName: '',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    };
    const name = buildFilename(fileMeta, docType, fmt);
    try {
      if (fmt === 'pdf')
        await exportPDF(
          output,
          name,
          docType,
          meta ?? undefined,
          templateId,
          atsMode,
          undefined,
          accent,
          letterLayoutId
        );
      else if (fmt === 'docx')
        await exportDOCX(
          output,
          name,
          docType,
          meta ?? undefined,
          templateId,
          atsMode,
          undefined,
          accent,
          letterLayoutId
        );
      else exportTXT(output, name);
    } catch (err) {
      console.error('[export] failed', {
        format: fmt,
        docType,
        error: errorClass(err),
      });
      notify.error({
        message: err instanceof Error && err.message ? err.message : t('common.exportFailed'),
      });
    }
  };

  return {
    state: session.state,
    busy: session.busy,
    starting: session.starting,
    currentStep,
    stageLabel,
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
