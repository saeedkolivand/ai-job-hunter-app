import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';
import type { ApplicationAnswer } from '@ajh/shared';

import {
  extractMetadata,
  generateApplicationAnswer,
  type GenerationMeta,
  researchCompany as fetchCompanyBrief,
} from '@/lib/generate';
import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';

/** Max length for a user-typed custom application question (chars, post-trim). */
export const MAX_CUSTOM_QUESTION_LEN = 500;

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
  // `guidance` is always undefined for a user-typed custom question — declared
  // here (not cast later) so `chosen` below is a uniform shape and reading
  // `q.guidance` needs no narrowing/assertion for either branch.
  const [custom, setCustom] = useState<{ id: string; question: string; guidance?: string }[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Snapshot of the last successful generate() context — lets updateAnswer re-save
  // a single rewritten answer without re-running the whole pipeline.
  const lastSaveContextRef = useRef<{
    detected: GenerationMeta;
    brief: string;
  } | null>(null);
  // Mirror of `answers` state for stable reads inside async callbacks without stale
  // closures. Kept in sync by an effect — never mutated inside a setAnswers updater
  // (updaters must be pure: they may run twice in React 19 StrictMode).
  const answersRef = useRef<Record<string, string>>({});
  useEffect(() => {
    answersRef.current = answers;
  }, [answers]);

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const addCustom = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || trimmed.length > MAX_CUSTOM_QUESTION_LEN) return;
    setCustom((prev) => [...prev, { id: crypto.randomUUID(), question: trimmed }]);
  };

  const removeCustom = (id: string) => setCustom((prev) => prev.filter((c) => c.id !== id));

  const canGenerate =
    canUse && hasDesc && resume.trim().length > 0 && (selected.size > 0 || custom.length > 0);

  const saveAnswers = async (
    detected: GenerationMeta,
    brief: string,
    results: ApplicationAnswer[]
  ) => {
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
  };

  const generate = async () => {
    if (!canGenerate || generating) return;
    setGenerating(true);
    setError(null);
    try {
      const detected = meta ?? (await extractMetadata(resume, jobDesc, model));
      const brief = researchCompany
        ? await fetchCompanyBrief(jobDesc, model, detected.companyName)
        : '';
      const chosen = [...APPLICATION_QUESTIONS.filter((q) => selected.has(q.id)), ...custom];
      const results: ApplicationAnswer[] = [];
      for (const q of chosen) {
        const answer = await generateApplicationAnswer({
          question: q.question,
          resume,
          jobAd: jobDesc,
          meta: detected,
          model,
          companyBrief: brief,
          // Only registry entries carry `guidance`; custom questions are
          // always `undefined` (see the `custom` state shape above).
          guidance: q.guidance,
        });
        results.push({ id: q.id, question: q.question, answer });
        setAnswers((prev) => ({ ...prev, [q.id]: answer }));
      }

      // Persist onto the per-job application record (merge-upsert by jobUrl), so
      // answers + brief live alongside the résumé/cover the tailor flow saved.
      await saveAnswers(detected, brief, results);
      lastSaveContextRef.current = { detected, brief };
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate answers');
    } finally {
      setGenerating(false);
    }
  };

  /**
   * Replace a single answer in local state WITHOUT persisting — used to revert an
   * optimistic update when the IPC save fails so the UI matches the stored truth.
   */
  const revertAnswer = (id: string, prev: string) => {
    setAnswers((current) => ({ ...current, [id]: prev }));
  };

  /**
   * Replace a single answer (from an AI rewrite) and re-persist the full answer
   * set through the same save path as generate(). No-op when no prior save context
   * exists (i.e. no generate has completed yet — the button is disabled in that case).
   * The caller is responsible for reverting via revertAnswer() if this rejects.
   */
  const updateAnswer = async (id: string, text: string) => {
    const ctx = lastSaveContextRef.current;
    if (!ctx) return;
    // Optimistic update — answersRef is synced by the effect after the render.
    setAnswers((prev) => ({ ...prev, [id]: text }));
    // Build the full answer list from the ref snapshot merged with the new value.
    // answersRef.current still holds the pre-update snapshot at this point (the
    // effect hasn't run yet), so we explicitly merge [id]: text on top.
    const allAnswers = Object.entries({ ...answersRef.current, [id]: text }).map(([qId, ans]) => {
      const q = APPLICATION_QUESTIONS.find((p) => p.id === qId) ?? custom.find((c) => c.id === qId);
      return { id: qId, question: q?.question ?? qId, answer: ans };
    });
    await saveAnswers(ctx.detected, ctx.brief, allAnswers);
  };

  return {
    selected,
    toggle,
    custom,
    addCustom,
    removeCustom,
    answers,
    generating,
    error,
    generate,
    canGenerate,
    updateAnswer,
    revertAnswer,
  };
}
