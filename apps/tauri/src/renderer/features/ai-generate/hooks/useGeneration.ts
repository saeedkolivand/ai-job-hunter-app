import type { AiGenerationSaveRequest } from '@ajh/shared/ipc';

import {
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
} from '@/lib/generate';

type AIGenerateStage = 'idle' | 'extracting' | 'configuring' | 'generating' | 'done';
type NotifyVariant = 'success' | 'error' | 'info' | 'warning';

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
  setStageLabel: (label: string) => void,
  /** Tracks an in-flight generation independently of `stage` — once the résumé is
   *  revealed (#23 progressive reveal) the stage is already `done` while the cover
   *  is still streaming, so "is generating" can't be derived from the stage. */
  setIsGenerating: (v: boolean) => void,
  /** Toast sink for success/failure notices (#23). */
  notify: (message: string, variant?: NotifyVariant) => void,
  /** Opt-in company research folded into the cover-letter prompt. */
  researchCompany = false,
  /**
   * Manual target-market override (a market id like `de`, or '' for auto). Passed
   * to the cover-letter prompt so the generated text matches the export layout,
   * which resolves the same market from this value + the detected job country.
   */
  marketOverride = ''
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
    setIsGenerating(true);
    setStage('generating');
    startStageRotation();

    const controller = new AbortController();
    abortControllerRef.current = controller;

    let finalResume = '';
    let finalCover = '';

    // Persist a finished generation (résumé and/or cover). Reused by the success
    // path and the "cover failed but the résumé is done" salvage path (#23).
    const persist = (resumeText: string, coverLetterText: string) =>
      saveAiGeneration.mutate({
        candidateName: meta.candidateName,
        jobTitle: meta.jobTitle,
        companyName: meta.companyName,
        resumeLanguage: meta.resumeLanguage,
        jobAdLanguage: meta.jobAdLanguage,
        targetLanguage: meta.targetLanguage,
        mismatch: meta.mismatch,
        topRequirements: meta.topRequirements,
        mode,
        resumeText,
        coverLetterText,
        jobAd,
      });

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
        // #23 progressive reveal: in a "both" run, surface the finished résumé
        // immediately and let the cover letter keep streaming in the background.
        if (target === 'both') {
          stopStageRotation();
          setStreamBuffer('');
          setActiveOut('resume');
          setStage('done');
        }
      }

      if (target === 'cover' || target === 'both') {
        // Cover-only keeps the streaming panel; in "both" the résumé is already
        // revealed and the cover streams into its tab in the done view.
        if (target === 'cover') setActiveOut('cover');
        setStreamBuffer('');
        setThinkingBuffer('');
        setModelLoading(true);
        tokenStartRef.current = null;
        setTokenCount(0);
        setGenStep({ current: total, total, label: 'Cover Letter' });
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
          onThink,
          { researchCompany, market: marketOverride || undefined }
        );
        setCoverOut(finalCover);
      }

      stopStageRotation();
      setStreamBuffer('');
      setGenStep(null);
      setStage('done');
      setActiveOut(target === 'cover' ? 'cover' : finalResume ? 'resume' : 'cover');

      persist(finalResume, finalCover);
      notify(
        target === 'both'
          ? t('aiGenerate.toast.bothReady')
          : target === 'cover'
            ? t('aiGenerate.toast.coverReady')
            : t('aiGenerate.toast.resumeReady'),
        'success'
      );
    } catch (err) {
      stopStageRotation();
      setStreamBuffer('');
      setGenStep(null);
      if (controller.signal.aborted) {
        // User cancelled — keep any finished document on screen, no error toast.
        setStage(finalResume || finalCover ? 'done' : 'configuring');
      } else if (target === 'both' && finalResume && !finalCover) {
        // The résumé finished but the cover letter failed — keep the résumé
        // visible (#23: never discard a finished document) and flag the cover.
        setStage('done');
        setActiveOut('resume');
        persist(finalResume, '');
        notify(t('aiGenerate.toast.coverFailed'), 'error');
      } else {
        setError(err instanceof Error ? err.message : t('aiGenerate.errors.generationFailed'));
        setStage('configuring');
        notify(t('aiGenerate.toast.failed'), 'error');
      }
    } finally {
      setIsGenerating(false);
      abortControllerRef.current = null;
    }
  };

  return { handleAnalyze, handleGenerate };
}
