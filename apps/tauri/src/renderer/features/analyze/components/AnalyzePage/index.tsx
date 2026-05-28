import { ScanSearch } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';

import type { DocumentRecord } from '@ajh/shared';

import { PageTransition } from '@/components/layout/PageTransition';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { AnalysisATSRisks } from '@/features/analyze/components/AnalysisATSRisks';
import { AnalysisLanguageMismatch } from '@/features/analyze/components/AnalysisLanguageMismatch';
import { AnalysisLanguageRecommendations } from '@/features/analyze/components/AnalysisLanguageRecommendations';
import { AnalysisMissingSkills } from '@/features/analyze/components/AnalysisMissingSkills';
import { AnalysisProgress } from '@/features/analyze/components/AnalysisProgress';
import { AnalysisRecommendations } from '@/features/analyze/components/AnalysisRecommendations';
import { AnalysisRewrites } from '@/features/analyze/components/AnalysisRewrites';
import { AnalysisScores } from '@/features/analyze/components/AnalysisScores';
import { AnalysisSectionAnalysis } from '@/features/analyze/components/AnalysisSectionAnalysis';
import { AnalysisSkills } from '@/features/analyze/components/AnalysisSkills';
import { AnalysisStrengths } from '@/features/analyze/components/AnalysisStrengths';
import { AnalysisVerdict } from '@/features/analyze/components/AnalysisVerdict';
import { AnalyzeLeftPanel } from '@/features/analyze/components/AnalyzeLeftPanel';
import { ACCEPTED_EXTS, MAX_BYTES } from '@/features/analyze/constants';
import { useAnalysisRun } from '@/features/analyze/hooks/useAnalysisRun';
import { useAnalyzeState } from '@/features/analyze/hooks/useAnalyzeState';
import { useTranslation } from '@/lib/i18n';
import type { AnalysisResult } from '@/lib/resume-ai';
import { useDocuments, useExtractText } from '@/services';
import { usePreferencesStore, usePromptQuality } from '@/store/preferences-store';

function AnalyzePage() {
  const { t, i18n } = useTranslation();

  const { resume, jobAd, stage, result, setResume, setJobAd, setStage, setResult } =
    useAnalyzeState();

  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
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
      const res = (await extractTextMutation.mutateAsync({ name: file.name, bytes })) as {
        text: string;
      };
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
  } = useAnalysisRun(resume, jobAd, selectedModel, canUseAI, i18n, setStage, setResult, t);

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
      <div className="flex h-full">
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
          onUpload={handleUpload}
          onReset={handleReset}
          onRun={run}
          setResume={setResume}
          setJobAd={setJobAd}
          setPromptQuality={setPromptQuality}
        />

        <div className="flex flex-1 flex-col overflow-hidden">
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
                <div className="flex-1 overflow-y-auto px-8 py-8">
                  <AnalysisProgress
                    running
                    stream={stream}
                    thinking={thinkingBuffer}
                    modelLoading={modelLoading}
                    tokenCount={tokenCount}
                    tokenStartMs={tokenStartRef.current}
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
                <div className="flex-1 overflow-y-auto px-8 py-8 space-y-4">
                  <AnalysisResults result={result} t={t} />
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {error && (
            <div className="shrink-0 mx-6 mb-4 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2.5 text-xs text-red-300/80">
              <div className="font-medium mb-0.5">{t('analyze.error')}</div>
              <div className="text-red-300/60">{error}</div>
            </div>
          )}
        </div>
      </div>
    </PageTransition>
  );
}

function AnalysisResults({ result, t }: { result: AnalysisResult; t: (key: string) => string }) {
  return (
    <>
      <AnalysisLanguageMismatch result={result} t={t} />
      <AnalysisScores result={result} t={t} />
      <AnalysisVerdict result={result} t={t} />
      <AnalysisStrengths result={result} t={t} />
      <AnalysisSkills result={result} t={t} />
      <AnalysisRecommendations result={result} t={t} />
      <AnalysisATSRisks result={result} t={t} />
      <AnalysisSectionAnalysis result={result} t={t} />
      <AnalysisRewrites result={result} />
      <AnalysisLanguageRecommendations result={result} t={t} />
      <AnalysisMissingSkills result={result} />
    </>
  );
}

export { AnalyzePage };
