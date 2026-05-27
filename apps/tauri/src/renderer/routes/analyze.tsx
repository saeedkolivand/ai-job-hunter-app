import {
  AlertCircle,
  AlertTriangle,
  Briefcase,
  RefreshCw,
  ScanSearch,
  Sparkles,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type { DocumentRecord } from '@ajh/shared';
import { Button } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { ModelSelector, useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ResumeInputCard } from '@/features/ai-workspace/components/ResumeInputCard';
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
import { CollapsibleInput } from '@/features/analyze/components/CollapsibleInput';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { type AnalysisResult, runAnalysis } from '@/lib/resume-ai';
import { useDocuments, useExtractText } from '@/services';
import type { PromptQuality } from '@/store/preferences-schema';
import { useOutputTone, usePreferencesStore, usePromptQuality } from '@/store/preferences-store';
import { useSessionStore } from '@/store/session-store';

export const Route = createFileRoute('/analyze')({ component: Analyze });

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const MAX_BYTES = 25 * 1024 * 1024;

type Stage = 'idle' | 'running' | 'done';

function Analyze() {
  const { t, i18n } = useTranslation();

  // Persistent state (survives navigation)
  const { analyze, setAnalyze } = useSessionStore();
  const { resume, jobAd, stage, result } = analyze;
  const setResume = useCallback((v: string) => setAnalyze({ resume: v }), [setAnalyze]);
  const setJobAd = (v: string) => setAnalyze({ jobAd: v });
  const setStage = (v: Stage) => setAnalyze({ stage: v });
  const setResult = (v: AnalysisResult | null) => setAnalyze({ result: v });

  // Transient state (resets each run)
  const [error, setError] = useState<string | null>(null);
  const [stream, setStream] = useState('');
  const [thinkingBuffer, setThinkingBuffer] = useState('');
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const [runId, setRunId] = useState(0);
  const [modelLoading, setModelLoading] = useState(false);
  const [tokenCount, setTokenCount] = useState(0);
  const tokenStartRef = useRef<number | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  const outputTone = useOutputTone();
  const promptQuality = usePromptQuality();
  const setPromptQuality = usePreferencesStore((s) => s.setPromptQuality);
  const extractTextMutation = useExtractText();
  const { data: documentsRaw = [] } = useDocuments();

  // Auto-fill default resume on mount (only when the field is empty)
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

  const run = async () => {
    if (!canRun) return;
    setRunId((n) => n + 1);
    setStage('running');
    setError(null);
    setResult(null);
    setStream('');
    setThinkingBuffer('');
    setModelLoading(true);
    setTokenCount(0);
    tokenStartRef.current = null;

    // Create new abort controller for this run
    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      const analysis = await runAnalysis({
        resume,
        jobAd,
        model: selectedModel,
        locale: i18n.language,
        meta: { targetLocale: i18n.language, outputTone: outputTone ?? 'professional' },
        onToken: (tok) => {
          if (!tokenStartRef.current) {
            tokenStartRef.current = Date.now();
          }
          setModelLoading(false);
          setTokenCount((c) => c + 1);
          setStream((p) => (p + tok).slice(-2000));
        },
        onThinking: (tok) => {
          setModelLoading(false);
          setThinkingBuffer((p) => p + tok);
        },
        signal: controller.signal,
      });
      setResult(analysis);
      setStage('done');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('analyze.errorBody'));
      setStage('idle');
    } finally {
      setStream('');
      abortControllerRef.current = null;
    }
  };

  const reset = async () => {
    // Abort running analysis if exists
    if (abortControllerRef.current && stage === 'running') {
      abortControllerRef.current.abort();
    }
    setResume('');
    setJobAd('');
    setResult(null);
    setError(null);
    setUploadError(null);
    setStage('idle');
    setRunId(0);
    extractTextMutation.reset();
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full">
        {/* ── LEFT PANEL — inputs + controls ───────────────────────────── */}
        <div className="flex w-[400px] shrink-0 flex-col border-r border-white/[0.05] overflow-y-auto">
          {/* Header */}
          <div className="px-6 pt-8 pb-4">
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2">
                <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-brand/15">
                  <ScanSearch size={14} className="text-brand-soft" />
                </div>
                <span className="text-base font-semibold text-foreground/90">
                  {t('analyze.title')}
                </span>
              </div>
              {stage !== 'idle' && (
                <Button
                  onClick={reset}
                  className="flex items-center gap-1 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
                >
                  <RefreshCw size={11} /> {t('analyze.reset')}
                </Button>
              )}
            </div>
            <p className="text-xs text-foreground/40">{t('analyze.subtitle')}</p>
          </div>

          {/* Model selector */}
          <div className="px-6 pb-4">
            <ModelSelector />
          </div>

          {/* Prompt quality selector */}
          <div className="px-6 pb-4">
            <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
              Prompt Quality
            </div>
            <div className="grid grid-cols-3 gap-1.5">
              {(
                [
                  { id: 'full' as PromptQuality, label: 'Full' },
                  { id: 'auto' as PromptQuality, label: 'Auto' },
                  { id: 'compact' as PromptQuality, label: 'Fast' },
                ] as const
              ).map(({ id, label }) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setPromptQuality(id)}
                  className={cn(
                    'flex items-center justify-center gap-1 rounded-lg border py-1.5 text-[11px] font-medium transition-all',
                    promptQuality === id
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
                  )}
                >
                  {id === 'compact' && <Zap size={11} />}
                  {label}
                </button>
              ))}
            </div>
            {promptQuality === 'compact' && (
              <div className="mt-2 flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2">
                <Zap size={11} className="text-amber-400 mt-0.5 shrink-0" />
                <p className="text-[10px] text-amber-400/80 leading-relaxed">
                  Fast mode — rewrites and detailed suggestions are reduced for speed.
                </p>
              </div>
            )}
            {promptQuality === 'full' && (
              <div className="mt-2 flex items-start gap-2 rounded-lg border border-orange-500/20 bg-orange-500/5 px-3 py-2">
                <AlertTriangle size={11} className="text-orange-400 mt-0.5 shrink-0" />
                <p className="text-[10px] text-orange-400/80 leading-relaxed">
                  Full mode on a small model may produce incomplete or noisy output.
                </p>
              </div>
            )}
          </div>

          {/* Inputs */}
          <div className="px-6 space-y-3 pb-4">
            <ResumeInputCard
              value={resume}
              onChange={setResume}
              onUpload={(f) => handleUpload('resume', f)}
              uploading={uploading === 'resume'}
              disabled={stage === 'running'}
              placeholder={t('analyze.resumePlaceholder')}
            />
            <CollapsibleInput
              label={t('analyze.jobAd')}
              icon={Briefcase}
              value={jobAd}
              onChange={setJobAd}
              uploading={uploading === 'jobAd'}
              onUpload={(f) => void handleUpload('jobAd', f)}
              placeholder={t('analyze.jobAdPlaceholder')}
              disabled={stage === 'running'}
              t={t}
            />
            {uploadError && (
              <div className="flex items-center gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-xs text-amber-200/80">
                <AlertCircle size={11} /> {uploadError}
              </div>
            )}
          </div>

          {/* CTA */}
          <div className="px-6 pb-6 mt-auto">
            {error && (
              <div className="mb-3 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2.5 text-xs text-red-300/80">
                <div className="font-medium mb-0.5">{t('analyze.error')}</div>
                <div className="text-red-300/60">{error}</div>
              </div>
            )}
            <Button
              size="md"
              variant={canRun && stage !== 'running' ? 'glass' : 'ghost'}
              onClick={() => void run()}
              loading={stage === 'running'}
              disabled={!canRun || stage === 'running'}
              className={cn('w-full justify-center', 'transition-all duration-150 ease-out')}
            >
              {stage !== 'running' && <Sparkles size={14} />}
              {stage === 'running'
                ? t('analyze.running')
                : stage === 'done'
                  ? t('analyze.reAnalyse')
                  : !canUseAI
                    ? aiReason === 'addApiKey'
                      ? t('analyze.addApiKey')
                      : t('analyze.selectModel')
                    : !canRun
                      ? t('analyze.pasteContent')
                      : t('analyze.run')}
            </Button>
          </div>
        </div>

        {/* ── RIGHT PANEL — state-driven output ────────────────────────── */}
        <div className="flex flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {/* Idle */}
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

            {/* Running */}
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

            {/* Done */}
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
        </div>
      </div>
    </PageTransition>
  );
}

// ─── Results ─────────────────────────────────────────────────────────────────

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
