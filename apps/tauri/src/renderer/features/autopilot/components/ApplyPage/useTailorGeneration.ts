import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMeta,
  type GenerationMode,
  PERSIST_DEBOUNCE_MS,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';
import { useUpdateAiGeneration } from '@/services/use-ai-generations';
import {
  EMPTY_SESSION,
  type GenerationResult,
  type TailorTarget,
  useGenerationStore,
} from '@/store/generation-store';

export type { TailorTarget };

/** Default résumé template for autopilot tailoring — seeds the sticky store
 *  preference (`applyTemplateId`). The live template/ATS now come from the results
 *  toolbar picker; preview and export read the SAME store values so the pre-download
 *  preview always equals the file (ADR-012). */
export const TAILOR_TEMPLATE: TemplateId = 'modern';
/** Default ATS single-column override — seeds the sticky store preference
 *  (`applyAtsMode`); the live value comes from the toolbar picker. */
export const TAILOR_ATS_MODE = false;
const MODE: GenerationMode = 'ats';

interface Params {
  /** Stable per-job session key (e.g. `autopilot:<jobUrl>`) so results survive
   *  closing/reopening the modal and navigating away. */
  contextId: string;
  jobDesc: string;
  model: string;
  canUse: boolean;
  hasDesc: boolean;
  /** The found job's URL — links the saved generation to it so the backend
   *  derives the "Applied" badge from a matching `jobUrl`. */
  jobUrl: string;
  /** The board the job came from (e.g. "linkedin"), stored on the record. */
  board: string;
  /** Opt-in: research the company and fold a brief into the cover-letter prompt. */
  researchCompany: boolean;
  /** Live résumé template id (from the sticky store preference) — drives the PDF/DOCX
   *  export so the file matches the toolbar preview. Render-time only; never regenerates. */
  templateId: TemplateId;
  /** Live ATS single-column override (from the sticky store preference) — must match
   *  the preview so the export is faithful. */
  atsMode: boolean;
}

/**
 * Thin adapter over the app-wide [`useGenerationStore`] for the ApplyPage: the
 * analyze → resume → cover flow runs in the store (background-safe), so this hook
 * only selects the session and keeps page-local UI state (copy/export). Leaving
 * the page no longer aborts — generation continues and reappears on return.
 */
export function useTailorGeneration({
  contextId,
  jobDesc,
  model,
  canUse,
  hasDesc,
  jobUrl,
  board,
  researchCompany,
  templateId,
  atsMode,
}: Params) {
  const { t } = useTranslation();
  const api = useAppClient();
  const qc = useQueryClient();
  const updateAiGeneration = useUpdateAiGeneration();

  const session = useGenerationStore((s) => s.sessions[contextId] ?? EMPTY_SESSION);
  const runTailor = useGenerationStore((s) => s.runTailor);
  const cancel = useGenerationStore((s) => s.cancel);
  const setActiveOutInStore = useGenerationStore((s) => s.setActiveOut);
  const setOutputInStore = useGenerationStore((s) => s.setOutput);
  const setSavedIdInStore = useGenerationStore((s) => s.setSavedId);

  const { generating, phase, resumeOut, coverOut, thinking, activeOut, error, meta, savedId } =
    session;

  // Modal-local, ephemeral UI — fine to reset when the modal remounts.
  const [copied, setCopied] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);

  // Debounced persistence of inline edits — one timer per field; flushed on unmount.
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

  const output = activeOut === 'resume' ? resumeOut : coverOut;

  // Persist the finished application linked to this job. Called by the store on a
  // clean run — bypasses the React Query mutation hook (which the modal may have
  // unmounted) and talks to the client directly, so a background generation still
  // records and flips the job to "Applied". Best-effort: a save failure never
  // surfaces over the already-shown output.
  const persist = ({ meta: m, resumeText, coverLetterText, companyBrief }: GenerationResult) => {
    void api.aiGenerations
      .save({
        candidateName: m.candidateName,
        jobTitle: m.jobTitle,
        companyName: m.companyName,
        resumeLanguage: m.resumeLanguage,
        jobAdLanguage: m.jobAdLanguage,
        targetLanguage: m.targetLanguage,
        mismatch: m.mismatch,
        topRequirements: m.topRequirements,
        mode: MODE,
        resumeText,
        coverLetterText,
        jobAd: jobDesc,
        jobUrl,
        board,
        companyBrief,
      })
      .then((res) => {
        // Stash the persisted id on the session so later inline edits can patch
        // this exact record (F1) without a separate save.
        if (res?.id) setSavedIdInStore(contextId, res.id);
        void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
        void qc.invalidateQueries({ queryKey: keys.autopilot.all });
      })
      .catch(() => {
        /* best-effort persistence — the generation is already shown to the user */
      });
  };

  // Inline edit (F1) of the active document. Updates the session immediately for a
  // smooth typing experience; once the record has been saved, debounced-persist
  // the edit to that record. Before a save lands (`savedId === null`) the edit is
  // session-only — the eventual `persist` will write the latest session text.
  const editActiveOutput = (text: string) => {
    setOutputInStore(contextId, activeOut, text);
    if (!savedId) return;
    const field = activeOut === 'resume' ? 'resume' : 'cover';
    const existing = persistTimers.current[field];
    if (existing) clearTimeout(existing);
    persistTimers.current[field] = setTimeout(() => {
      updateAiGeneration.mutate(
        activeOut === 'resume'
          ? { id: savedId, resumeText: text }
          : { id: savedId, coverLetterText: text }
      );
    }, PERSIST_DEBOUNCE_MS);
  };

  const generate = (resume: string, target: TailorTarget) => {
    if (!canUse || !hasDesc) return Promise.resolve();
    return runTailor({
      contextId,
      resume,
      jobDesc,
      model,
      mode: MODE,
      target,
      researchCompany,
      t,
      onComplete: persist,
    });
  };

  const abort = () => cancel(contextId);
  const setActiveOut = (which: 'resume' | 'cover') => setActiveOutInStore(contextId, which);

  const phaseLabel =
    phase === 'analyzing'
      ? t('autopilot.apply.analyzing')
      : phase === 'resume'
        ? t('autopilot.apply.writingResume')
        : phase === 'cover'
          ? t('autopilot.apply.writingCover')
          : '';

  const copy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
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
    if (fmt === 'pdf')
      await exportPDF(output, name, docType, meta ?? undefined, templateId, atsMode);
    else if (fmt === 'docx')
      await exportDOCX(output, name, docType, meta ?? undefined, templateId, atsMode);
    else exportTXT(output, name);
  };

  return {
    generating,
    phase,
    phaseLabel,
    thinking,
    resumeOut,
    coverOut,
    activeOut,
    setActiveOut,
    output,
    error,
    copied,
    exportOpen,
    setExportOpen,
    generate,
    abort,
    copy,
    exportAs,
    // Inline edit of the active document (F1) — session-immediate + debounced persist.
    editActiveOutput,
    // Detected metadata — lets the questions assistant reuse it instead of re-extracting.
    meta,
  };
}
