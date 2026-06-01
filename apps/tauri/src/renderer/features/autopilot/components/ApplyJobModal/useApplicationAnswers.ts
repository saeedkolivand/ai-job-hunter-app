import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';
import type { ApplicationAnswer } from '@ajh/shared/ipc';

import {
  extractMetadata,
  generateApplicationAnswer,
  type GenerationMeta,
  researchCompany as fetchCompanyBrief,
} from '@/lib/generate';
import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';

interface Params {
  resume: string;
  jobDesc: string;
  model: string;
  /** Reuse the same opt-in research toggle as the cover letter (shared brief). */
  researchCompany: boolean;
  /** Metadata already detected by the tailor flow — skips a re-extract when set. */
  meta?: GenerationMeta | null;
  canUse: boolean;
  hasDesc: boolean;
  /** Links the answers to the per-job application record (merge-upsert by url). */
  jobUrl: string;
  board: string;
}

/**
 * Drafts résumé-grounded answers to a user-selected set of application questions.
 * Detects metadata once (reusing the tailor flow's when available), fetches the
 * company brief once when research is on (server-cached, so it dedupes with the
 * cover letter's), then answers each selected question through the shared
 * grounded pipeline — sequentially, filling answers in as they complete.
 */
export function useApplicationAnswers({
  resume,
  jobDesc,
  model,
  researchCompany,
  meta,
  canUse,
  hasDesc,
  jobUrl,
  board,
}: Params) {
  const api = useAppClient();
  const qc = useQueryClient();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const canGenerate = canUse && hasDesc && selected.size > 0 && resume.trim().length > 0;

  const generate = async () => {
    if (!canGenerate || generating) return;
    setGenerating(true);
    setError(null);
    try {
      const detected = meta ?? (await extractMetadata(resume, jobDesc, model));
      const brief = researchCompany ? await fetchCompanyBrief(jobDesc, model) : '';
      const chosen = APPLICATION_QUESTIONS.filter((q) => selected.has(q.id));
      const results: ApplicationAnswer[] = [];
      for (const q of chosen) {
        const answer = await generateApplicationAnswer({
          question: q.question,
          resume,
          jobAd: jobDesc,
          meta: detected,
          model,
          companyBrief: brief,
        });
        results.push({ id: q.id, question: q.question, answer });
        setAnswers((prev) => ({ ...prev, [q.id]: answer }));
      }

      // Persist onto the per-job application record (merge-upsert by jobUrl), so
      // answers + brief live alongside the résumé/cover the tailor flow saved.
      await api.aiGenerations.save({
        candidateName: detected.candidateName,
        jobTitle: detected.jobTitle,
        companyName: detected.companyName,
        resumeLanguage: detected.resumeLanguage,
        jobAdLanguage: detected.jobAdLanguage,
        targetLanguage: detected.targetLanguage,
        mismatch: detected.mismatch,
        topRequirements: detected.topRequirements,
        mode: 'ats',
        resumeText: '',
        coverLetterText: '',
        jobAd: jobDesc,
        jobUrl,
        board,
        applicationAnswers: results,
        companyBrief: brief,
      });
      void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
      void qc.invalidateQueries({ queryKey: keys.autopilot.all });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate answers');
    } finally {
      setGenerating(false);
    }
  };

  return { selected, toggle, answers, generating, error, generate, canGenerate };
}
