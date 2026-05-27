import type { AiGenerationSaveRequest } from '@ajh/shared/ipc';

import {
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
} from '@/lib/generate-ai';

type AIGenerateStage = 'idle' | 'extracting' | 'configuring' | 'generating' | 'done';

export function useGeneration(
  resume: string,
  jobAd: string,
  meta: GenerationMeta | null,
  mode: GenerationMode,
  target: 'resume' | 'cover' | 'both',
  selectedModel: string,
  setStage: (stage: AIGenerateStage) => void,
  setMeta: (meta: GenerationMeta | null) => void,
  setResumeOut: (out: string | ((p: string) => string)) => void,
  setCoverOut: (out: string | ((p: string) => string)) => void,
  setActiveOut: (out: 'resume' | 'cover') => void,
  setStreamBuffer: (buf: string | ((prev: string) => string)) => void,
  setThinkingBuffer: (buf: string | ((prev: string) => string)) => void,
  setModelLoading: (loading: boolean) => void,
  setTokenCount: (count: number | ((c: number) => number)) => void,
  setGenStep: (step: { current: number; total: number; label: string } | null) => void,
  setError: (error: string | null) => void,
  tokenStartRef: React.MutableRefObject<number | null>,
  startStageRotation: () => void,
  stopStageRotation: () => void,
  abortControllerRef: React.MutableRefObject<AbortController | null>,
  saveAiGeneration: { mutate: (data: AiGenerationSaveRequest) => void },
  t: (key: string) => string,
  setStageLabel: (label: string) => void
) {
  const handleAnalyze = async () => {
    setError(null);
    setStage('extracting');
    setStageLabel(t('aiGenerate.analyzingDocuments'));
    try {
      const detected = await extractMetadata(resume, jobAd, selectedModel);
      setMeta(detected);
      setStage('configuring');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('aiGenerate.errors.extractionFailed'));
      setStage('idle');
    }
  };

  const handleGenerate = async () => {
    if (!meta || !selectedModel) return;
    setError(null);
    setResumeOut('');
    setCoverOut('');
    setStreamBuffer('');
    setThinkingBuffer('');
    setModelLoading(true);
    setTokenCount(0);
    tokenStartRef.current = null;
    const total = target === 'both' ? 2 : 1;
    setGenStep({ current: 1, total, label: target === 'cover' ? 'Cover Letter' : 'Resume' });
    setStage('generating');
    startStageRotation();

    const controller = new AbortController();
    abortControllerRef.current = controller;

    let finalResume = '';
    let finalCover = '';

    const onTok =
      (setter: (fn: (p: string) => string) => void, accumulate: (t: string) => void) =>
      (tok: string) => {
        if (!tokenStartRef.current) tokenStartRef.current = Date.now();
        setModelLoading(false);
        setTokenCount((c) => c + 1);
        accumulate(tok);
        setter((p) => (p + tok).slice(-600));
      };

    const onThink = (tok: string) => {
      setModelLoading(false);
      setThinkingBuffer((p) => p + tok);
    };

    try {
      if (target === 'resume' || target === 'both') {
        setActiveOut('resume');
        setStreamBuffer('');
        setThinkingBuffer('');
        finalResume = await generateResume(
          resume,
          jobAd,
          meta,
          mode,
          selectedModel,
          onTok(setStreamBuffer, (t) => {
            setResumeOut((p) => p + t);
          }),
          undefined,
          controller.signal,
          onThink
        );
        setResumeOut(finalResume);
      }

      if (target === 'cover' || target === 'both') {
        setActiveOut('cover');
        setStreamBuffer('');
        setThinkingBuffer('');
        setModelLoading(true);
        tokenStartRef.current = null;
        setTokenCount(0);
        setGenStep({ current: 2, total: 2, label: 'Cover Letter' });
        finalCover = await generateCoverLetter(
          resume,
          jobAd,
          meta,
          mode,
          selectedModel,
          onTok(setStreamBuffer, (t) => {
            setCoverOut((p) => p + t);
          }),
          undefined,
          controller.signal,
          onThink
        );
        setCoverOut(finalCover);
      }

      stopStageRotation();
      setStreamBuffer('');
      setStage('done');
      const doneActiveOut =
        target === 'cover' ? 'cover' : finalResume ? 'resume' : finalCover ? 'cover' : 'resume';
      setActiveOut(doneActiveOut);

      void saveAiGeneration.mutate({
        candidateName: meta.candidateName,
        jobTitle: meta.jobTitle,
        companyName: meta.companyName,
        resumeLanguage: meta.resumeLanguage,
        jobAdLanguage: meta.jobAdLanguage,
        targetLanguage: meta.targetLanguage,
        mismatch: meta.mismatch,
        topRequirements: meta.topRequirements,
        mode,
        resumeText: finalResume,
        coverLetterText: finalCover,
        jobAd,
      });
    } catch (err) {
      stopStageRotation();
      setError(err instanceof Error ? err.message : t('aiGenerate.errors.generationFailed'));
      setStage('configuring');
    } finally {
      abortControllerRef.current = null;
    }
  };

  return { handleAnalyze, handleGenerate };
}
