import { ScanSearch } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ErrorState } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { useCanUseAI, useSelectedModel, useSelectedProvider } from '@/components/ui/ModelSelector';
import { AnalysisProgress } from '@/features/analyze/components/AnalysisProgress';
import { AnalysisResults } from '@/features/analyze/components/AnalysisResults';
import { AnalyzeLeftPanel } from '@/features/analyze/components/AnalyzeLeftPanel';
import { ACCEPTED_EXTS, MAX_BYTES } from '@/features/analyze/constants';
import { useAnalysisRun } from '@/features/analyze/hooks/useAnalysisRun';
import { useAnalyzeState } from '@/features/analyze/hooks/useAnalyzeState';
import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useDocuments, useExtractText } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import { usePreferencesStore, usePromptQuality } from '@/store/preferences-store';

function AnalyzePage() {
  const { t, i18n } = useTranslation();

  const {
    resume,
    jobAd,
    stage,
    result,
    analysisMode,
    setResume,
    setJobAd,
    setStage,
    setResult,
    setAnalysisMode,
  } = useAnalyzeState();

  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  // Provider-aware progress estimate: CLI agents & local models are much slower
  // than cloud APIs, so the progress bar/ETA shouldn't promise ~50s for them.
  const activeProvider = useSelectedProvider();
  const providerKind = PROVIDERS[activeProvider as AiProvider]?.kind ?? 'local-server';
  const analysisEstimateMs = providerKind === 'cloud' ? 35_000 : 90_000;
  const slowProvider = providerKind !== 'cloud';
  const promptQuality = usePromptQuality();
  const setPromptQuality = usePreferencesStore((s) => s.setPromptQuality);
  const extractTextMutation = useExtractText();
  const { data: documentsRaw = [] } = useDocuments();

  // Auto-fill default resume on mount
  useEffect(() => {
    if (resume) return;
    const docs = documentsRaw as Array<DocumentRecord & { _id?: string; text?: string }>;
    const defaultDoc = docs.find((d) => d.isDefault) ?? docs[0];
    const text = defaultDoc?.text?.trim();
    if (text) setResume(text);
  }, [documentsRaw, resume, setResume]);

  const handleUpload = async (target: 'resume' | 'jobAd', file: File) => {
    setUploadError(null);
    const ext = file.name.toLowerCase().split('.').pop() ?? '';
    if (!ACCEPTED_EXTS.includes(ext as (typeof ACCEPTED_EXTS)[number])) {
      setUploadError(t('analyze.upload.unsupported', { ext }));
      return;
    }
    if (file.size > MAX_BYTES) {
      setUploadError(t('analyze.upload.tooLarge'));
      return;
    }
    setUploading(target);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const res = await extractTextMutation.mutateAsync({ name: file.name, bytes });
      const text = (res?.text ?? '').trim();
      if (!text) {
        setUploadError(t('analyze.upload.empty'));
        return;
      }
      if (target === 'resume') setResume(text);
      else setJobAd(text);
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : t('analyze.upload.failed'));
    } finally {
      setUploading(null);
    }
  };

  const canRun = useMemo(() => {
    const hasContent = resume.trim().length > 50 && jobAd.trim().length > 50;
    return hasContent && canUseAI;
  }, [resume, jobAd, canUseAI]);

  const {
    error,
    stream,
    thinkingBuffer,
    runId,
    modelLoading,
    tokenCount,
    tokenStartRef,
    run,
    reset,
  } = useAnalysisRun(
    resume,
    jobAd,
    selectedModel,
    canUseAI,
    i18n,
    setStage,
    setResult,
    t,
    analysisMode
  );

  const handleReset = async () => {
    await reset();
    setResume('');
    setJobAd('');
    setResult(null);
    setUploadError(null);
    setStage('idle');
    extractTextMutation.reset();
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full flex-col md:flex-row">
        <AnalyzeLeftPanel
          resume={resume}
          jobAd={jobAd}
          stage={stage}
          uploading={uploading}
          uploadError={uploadError}
          canRun={canRun}
          canUseAI={canUseAI}
          aiReason={aiReason ?? ''}
          promptQuality={promptQuality}
          analysisMode={analysisMode}
          onUpload={handleUpload}
          onReset={handleReset}
          onRun={run}
          setResume={setResume}
          setJobAd={setJobAd}
          setPromptQuality={setPromptQuality}
          setAnalysisMode={setAnalysisMode}
        />

        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {stage === 'idle' && (
              <motion.div
                key="idle"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="flex flex-1 flex-col items-center justify-center gap-6 px-10"
              >
                <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/20">
                  <ScanSearch size={36} className="text-brand-soft/60" />
                </div>
                <div className="text-center">
                  <div className="text-lg font-semibold text-foreground/50">
                    {t('analyze.idleTitle')}
                  </div>
                  <div className="mt-1 text-sm text-foreground/30">{t('analyze.idleSubtitle')}</div>
                </div>
                <div className="flex flex-col gap-2 text-center">
                  {[
                    t('analyze.features.ats'),
                    t('analyze.features.keywords'),
                    t('analyze.features.feedback'),
                    t('analyze.features.verdict'),
                  ].map((f, i) => (
                    <div key={i} className="flex items-center gap-2 text-xs text-foreground/35">
                      <span className="h-1 w-1 rounded-full bg-brand/40" /> {f}
                    </div>
                  ))}
                </div>
              </motion.div>
            )}

            {stage === 'running' && (
              <motion.div
                key="running"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="flex flex-1 flex-col overflow-hidden"
              >
                <div className="select-text flex-1 overflow-y-auto px-8 py-8">
                  <AnalysisProgress
                    running
                    stream={stream}
                    thinking={thinkingBuffer}
                    modelLoading={modelLoading}
                    tokenCount={tokenCount}
                    tokenStartMs={tokenStartRef.current}
                    estimatedMs={analysisEstimateMs}
                    slow={slowProvider}
                    t={t}
                  />
                </div>
              </motion.div>
            )}

            {stage === 'done' && result && (
              <motion.div
                key={`done-${runId}`}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-1 flex-col overflow-hidden"
              >
                <div className="@container select-text flex-1 overflow-y-auto px-8 py-8 space-y-4">
                  <AnalysisResults result={result} t={t} />
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {error && (
            <div className="shrink-0 mx-6 mb-4">
              <ErrorState
                title={t('analyze.error')}
                description={error}
                onRetry={() => void run()}
                className="rounded-lg border border-red-400/20 bg-red-400/5 py-6"
              />
            </div>
          )}
        </div>
      </div>
    </PageTransition>
  );
}

export { AnalyzePage };
