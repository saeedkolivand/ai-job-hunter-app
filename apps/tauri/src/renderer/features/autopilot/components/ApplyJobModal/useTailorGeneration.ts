import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMeta,
  type GenerationMode,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';
import {
  EMPTY_SESSION,
  type GenerationResult,
  type TailorTarget,
  useGenerationStore,
} from '@/store/generation-store';

export type { TailorTarget };

const TEMPLATE: TemplateId = 'modern';
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
}

/**
 * Thin adapter over the app-wide [`useGenerationStore`] for ApplyJobModal: the
 * analyze → resume → cover flow runs in the store (background-safe), so this hook
 * only selects the session and keeps modal-local UI state (copy/export). Closing
 * the modal no longer aborts — generation continues and reappears on reopen.
 */
export function useTailorGeneration({
  contextId,
  jobDesc,
  model,
  canUse,
  hasDesc,
  jobUrl,
  board,
}: Params) {
  const { t } = useTranslation();
  const api = useAppClient();
  const qc = useQueryClient();

  const session = useGenerationStore((s) => s.sessions[contextId] ?? EMPTY_SESSION);
  const runTailor = useGenerationStore((s) => s.runTailor);
  const cancel = useGenerationStore((s) => s.cancel);
  const setActiveOutInStore = useGenerationStore((s) => s.setActiveOut);

  const { generating, phase, resumeOut, coverOut, thinking, activeOut, error, meta } = session;

  // Modal-local, ephemeral UI — fine to reset when the modal remounts.
  const [copied, setCopied] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);

  const output = activeOut === 'resume' ? resumeOut : coverOut;

  // Persist the finished application linked to this job. Called by the store on a
  // clean run — bypasses the React Query mutation hook (which the modal may have
  // unmounted) and talks to the client directly, so a background generation still
  // records and flips the job to "Applied". Best-effort: a save failure never
  // surfaces over the already-shown output.
  const persist = ({ meta: m, resumeText, coverLetterText }: GenerationResult) => {
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
      })
      .then(() => {
        void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
        void qc.invalidateQueries({ queryKey: keys.autopilot.all });
      })
      .catch(() => {
        /* best-effort persistence — the generation is already shown to the user */
      });
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
    if (fmt === 'pdf') await exportPDF(output, name, docType, meta ?? undefined, TEMPLATE, false);
    else if (fmt === 'docx')
      await exportDOCX(output, name, docType, meta ?? undefined, TEMPLATE, false);
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
  };
}
