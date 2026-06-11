import { useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { useToast } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ROUTES } from '@/constants/routes';
import { type GenerationMeta, synthesizeResume } from '@/lib/generate';
import { useSaveAiGeneration } from '@/services/use-ai-generations';
import { useContactProfile } from '@/services/use-contact-profile';
import { useSessionStore } from '@/store/session-store';

/**
 * Resume Builder orchestration (#1 / B9): builds the synthesis `GenerationMeta`
 * from the interview answers + saved contact profile, runs the single streamed
 * synthesis pass, persists the result to Documents (saveAiGeneration), and
 * provides the in-memory "tailor to a job" handoff into AI-Generate. State lives
 * in the `resumeBuilder` session slice; transient streaming state is local.
 */
export function useResumeBuilder() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const notify = useToast();
  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  const { data: contact } = useContactProfile();
  const saveAiGeneration = useSaveAiGeneration();

  const resumeBuilder = useSessionStore((s) => s.resumeBuilder);
  const setResumeBuilder = useSessionStore((s) => s.setResumeBuilder);
  const resetResumeBuilder = useSessionStore((s) => s.resetResumeBuilder);
  const setAIGenerate = useSessionStore((s) => s.setAIGenerate);
  const resetAIGenerate = useSessionStore((s) => s.resetAIGenerate);

  const { answers, language, locale, templateId, atsMode, stage, output } = resumeBuilder;

  const [streamBuffer, setStreamBuffer] = useState('');
  const [thinkingBuffer, setThinkingBuffer] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [modelLoading, setModelLoading] = useState(false);
  const [tokenCount, setTokenCount] = useState(0);
  const tokenStartRef = useRef<number | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Name comes from the authoritative contact profile (the export header source);
  // fall back to anything captured directly in the answers.
  const fullName = contact?.fullName?.trim() || answers.fullName?.trim() || '';

  const meta: GenerationMeta = {
    resumeLanguage: language,
    jobAdLanguage: language,
    mismatch: false,
    candidateName: fullName,
    jobTitle: answers.headline?.trim() || '',
    companyName: '',
    targetLanguage: language,
    topRequirements: [],
  };

  // Completeness gate: a real résumé needs a name, some history, and some skills.
  const hasName = fullName.length > 0;
  const hasHistory =
    (answers.experience?.some((e) => e.title?.trim() || e.company?.trim()) ?? false) ||
    (answers.education?.some((e) => e.degree?.trim() || e.institution?.trim()) ?? false);
  const hasSkills = answers.skills?.some((s) => s.trim()) ?? false;
  const isComplete = hasName && hasHistory && hasSkills;

  // The inline-validation gate (bad URLs/years, half-filled entries) now lives in
  // the react-hook-form layer (`builderSchema` + the wizard's `formState.isValid`).
  // This hook only owns the completeness + AI-availability portion of the gate.
  const canGenerate = isComplete && canUseAI;

  const synthesize = async () => {
    if (isGenerating) return;
    setError(null);
    setStreamBuffer('');
    setThinkingBuffer('');
    setTokenCount(0);
    setModelLoading(true);
    tokenStartRef.current = null;
    setIsGenerating(true);
    setResumeBuilder({ stage: 'generating' });

    const controller = new AbortController();
    abortRef.current = controller;

    try {
      const result = await synthesizeResume(
        { ...answers, fullName },
        meta,
        selectedModel,
        (tok) => {
          if (!tokenStartRef.current) tokenStartRef.current = Date.now();
          setModelLoading(false);
          setTokenCount((c) => c + 1);
          setStreamBuffer((p) => (p + tok).slice(-600));
        },
        language,
        controller.signal,
        (tok) => {
          setModelLoading(false);
          setThinkingBuffer((p) => p + tok);
        }
      );

      setResumeBuilder({ output: result, stage: 'done' });
      // Persist as a generation so it appears in Documents (no jobUrl → a fresh
      // row, never a per-job merge). No new IPC.
      saveAiGeneration.mutate({
        candidateName: meta.candidateName,
        jobTitle: meta.jobTitle,
        companyName: '',
        resumeLanguage: language,
        jobAdLanguage: language,
        targetLanguage: language,
        mismatch: false,
        topRequirements: [],
        mode: 'ats',
        resumeText: result,
        coverLetterText: '',
        jobAd: '',
      });
      notify(t('build.toast.done'), 'success');
    } catch (err) {
      // A user-cancelled run is silent (Start over / unmount aborts in-flight).
      if (!controller.signal.aborted) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        setResumeBuilder({ stage: 'interview' });
        notify(t('build.toast.failed'), 'error');
      }
    } finally {
      setIsGenerating(false);
      abortRef.current = null;
    }
  };

  /**
   * "Tailor to a job" handoff (#1 grill Q5): seed the built résumé straight into
   * AI-Generate's input in-memory and navigate there. The base-résumé picker only
   * lists imported DocumentRecords, so this avoids a text-save IPC.
   */
  const tailorToJob = () => {
    if (!output.trim()) return;
    resetAIGenerate();
    setAIGenerate({ resume: output });
    void navigate({ to: ROUTES.GENERATE });
  };

  const reset = () => {
    if (abortRef.current && isGenerating) abortRef.current.abort();
    setError(null);
    setStreamBuffer('');
    setThinkingBuffer('');
    resetResumeBuilder();
  };

  return {
    answers,
    language,
    locale,
    templateId,
    atsMode,
    stage,
    output,
    setResumeBuilder,
    meta,
    fullName,
    canGenerate,
    canUseAI,
    aiReason: aiReason ?? '',
    isComplete,
    isGenerating,
    streamBuffer,
    thinkingBuffer,
    modelLoading,
    tokenCount,
    tokenStartMs: tokenStartRef.current,
    error,
    setError,
    synthesize,
    tailorToJob,
    reset,
  };
}
