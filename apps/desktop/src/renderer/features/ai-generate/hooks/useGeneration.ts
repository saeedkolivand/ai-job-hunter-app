import type { AiGenerationSaveRequest } from '@ajh/shared/ipc';
import type { NotificationApi } from '@ajh/ui';

import { errorDetail } from '@/lib/error-class';
import {
  computeQualityReport,
  type EmphasisId,
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  type QualityReport,
  serializeQualityReport,
} from '@/lib/generate';
import { resolveActiveProvider } from '@/lib/generate/provider-context';

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
  /** Deterministic content-quality report for the just-finished generation
   *  (computed right before persisting) — additive sibling to `setMeta`. */
  setReport: (report: QualityReport | null) => void,
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
  /** Notification API for success/failure notices (#23). */
  notify: NotificationApi,
  /** Opt-in company research folded into the cover-letter prompt. */
  researchCompany = false,
  /**
   * Manual target-market override (a market id like `de`, or '' for auto). Passed
   * to the cover-letter prompt so the generated text matches the export layout,
   * which resolves the same market from this value + the detected job country.
   */
  marketOverride = '',
  /** User-selected emphasis directives (#15), merged into meta at generate-time. */
  emphasis: EmphasisId[] = [],
  /**
   * URL-import provenance (ADR-031): the source url + board when the job ad came
   * from a URL import. Persisted onto the save request so the generation joins
   * applied-detection + cluster provenance. Absent (undefined) for pasted or
   * uploaded text — we never invent a url for text the user typed.
   */
  jobUrl?: string,
  board?: string
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
    // Fold the user's emphasis directives (#15) into the meta the prompt builders
    // read. Kept separate from the stored `meta` (extracted) so the wizard owns it.
    const genMeta: GenerationMeta = emphasis.length ? { ...meta, emphasis } : meta;
    setError(null);
    // Clear the previous generation's report too — otherwise the 'both'-mode
    // progressive-reveal window (stage 'done' while the cover still streams)
    // renders the PREVIOUS run's report/evidence against the NEW résumé until
    // `persist` recomputes one.
    setReport(null);
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

    // A prior run may still be persisting (validating) when Regenerate is
    // clicked again — abort its controller so `persist`'s guard below can tell
    // its work is stale and skip it, instead of racing this fresh run's save.
    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    // Log the RESOLVED model, not `selectedModel` — the latter is only the
    // fallback `resolveActiveProvider` uses when the active config has none, so
    // a diagnostics bundle would otherwise name a model the run never used.
    const { activeProvider: provider, activeModel } = resolveActiveProvider(selectedModel);
    const startedAt = Date.now();
    console.warn('[handleGenerate] start', { provider, model: activeModel, target });

    let finalResume = '';
    let finalCover = '';
    // Company-research brief that informed the cover letter — persisted so the doc
    // card's "Company research" section shows. '' when research is off / cover failed.
    let finalCompanyBrief = '';

    // Persist a finished generation (résumé and/or cover). Reused by the success
    // path and the "cover failed but the résumé is done" salvage path (#23).
    // Awaited so the report lands in the same save it describes, but content
    // validation is best-effort and can never THROW (`computeQualityReport`
    // degrades failures to `null` internally) — so it can delay this save by at
    // most one fast, local IPC round-trip, never block or fail it.
    // Returns `false` (and skips its own writes) when a NEWER run superseded
    // this one while validation was in flight (Regenerate/reset) — see the
    // `abort()` above and `reset()`'s abort in AIGeneratePage.
    const persist = async (
      resumeText: string,
      coverLetterText: string,
      companyBrief: string
    ): Promise<boolean> => {
      const report = await computeQualityReport({
        sourceResume: resume,
        jobAd,
        topRequirements: meta.topRequirements,
        targetLanguage: meta.targetLanguage,
        resumeText,
        coverLetterText,
      });
      if (controller.signal.aborted) return false;
      setReport(report);
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
        companyBrief,
        // ADR-031: carry URL-import provenance ONLY when it exists — absent for
        // pasted/uploaded text (never fabricate a url/board).
        ...(jobUrl ? { jobUrl } : {}),
        ...(board ? { board } : {}),
        ...(report ? { qualityReport: serializeQualityReport(report) } : {}),
      });
      return true;
    };

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
          genMeta,
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
        const cover = await generateCoverLetter(
          resume,
          jobAd,
          genMeta,
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
        finalCover = cover.text;
        finalCompanyBrief = cover.companyBrief;
        setCoverOut(finalCover);
      }

      console.warn('[handleGenerate] done', {
        target,
        resumeLength: finalResume.length,
        coverLength: finalCover.length,
        durationMs: Date.now() - startedAt,
      });

      stopStageRotation();
      setStreamBuffer('');
      setGenStep(null);
      setStage('done');
      setActiveOut(target === 'cover' ? 'cover' : finalResume ? 'resume' : 'cover');

      const committed = await persist(finalResume, finalCover, finalCompanyBrief);
      // A newer run superseded this one during validation — its own success
      // path owns the toast; showing one here would misreport WHICH run finished.
      if (!committed) return;
      notify.success({
        message:
          target === 'both'
            ? t('aiGenerate.toast.bothReady')
            : target === 'cover'
              ? t('aiGenerate.toast.coverReady')
              : t('aiGenerate.toast.resumeReady'),
      });
    } catch (err) {
      // Ownership guard (mirrors the `finally` guard below): a Regenerate click
      // may have already aborted this run's controller and started its own —
      // the newer run owns the UI now, so a stale run's late rejection must
      // never stomp its in-flight state (stage/buffers/stage rotation).
      if (abortControllerRef.current !== controller) {
        console.warn('[handleGenerate] superseded, error handling skipped', { target });
        return;
      }
      stopStageRotation();
      setStreamBuffer('');
      setGenStep(null);
      if (controller.signal.aborted) {
        console.warn('[handleGenerate] cancelled', { target });
        // User cancelled — keep any finished document on screen, no error toast.
        setStage(finalResume || finalCover ? 'done' : 'configuring');
      } else if (target === 'both' && finalResume && !finalCover) {
        console.warn('[handleGenerate] cover failed, resume kept', {
          error: err instanceof Error ? err.message : String(err),
        });
        // The résumé finished but the cover letter failed — keep the résumé
        // visible (#23: never discard a finished document) and flag the cover.
        setStage('done');
        setActiveOut('resume');
        if (await persist(finalResume, '', '')) {
          notify.error({ message: t('aiGenerate.toast.coverFailed') });
        }
      } else {
        // Capped, not classed: unlike an export failure (whose reason the Rust
        // side logs as issue codes) a generation failure has no second record
        // of WHY. See `errorDetail`.
        console.error('[handleGenerate] failed', { target, error: errorDetail(err) });
        setError(err instanceof Error ? err.message : t('aiGenerate.errors.generationFailed'));
        setStage('configuring');
        notify.error({ message: t('aiGenerate.toast.failed') });
      }
    } finally {
      // Ownership guard: a Regenerate/reset click may already have replaced
      // this run's controller (see the `abort()` above) — only the run that
      // still owns the ref gets to flip `isGenerating` off / clear it, so a
      // stale run's cleanup can never stomp on a newer run's in-flight state.
      if (abortControllerRef.current === controller) {
        setIsGenerating(false);
        abortControllerRef.current = null;
      }
    }
  };

  return { handleAnalyze, handleGenerate };
}
