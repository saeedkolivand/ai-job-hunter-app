import {
  AlertCircle,
  Briefcase,
  CheckCircle2,
  ChevronDown,
  RefreshCw,
  RotateCcw,
  ScanSearch,
  Sparkles,
  Upload,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';

import type { DocumentRecord } from '@ajh/shared';
import { Button, TextArea } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { ResumeInputCard } from '@/features/ai-workspace/components/ResumeInputCard';
import { AnalysisATSRisks } from '@/features/analyze/components/AnalysisATSRisks';
import { AnalysisLanguageMismatch } from '@/features/analyze/components/AnalysisLanguageMismatch';
import { AnalysisLanguageRecommendations } from '@/features/analyze/components/AnalysisLanguageRecommendations';
import { AnalysisMissingSkills } from '@/features/analyze/components/AnalysisMissingSkills';
import { AnalysisRecommendations } from '@/features/analyze/components/AnalysisRecommendations';
import { AnalysisRewrites } from '@/features/analyze/components/AnalysisRewrites';
import { AnalysisScores } from '@/features/analyze/components/AnalysisScores';
import { AnalysisSectionAnalysis } from '@/features/analyze/components/AnalysisSectionAnalysis';
import { AnalysisSkills } from '@/features/analyze/components/AnalysisSkills';
import { AnalysisStrengths } from '@/features/analyze/components/AnalysisStrengths';
import { AnalysisVerdict } from '@/features/analyze/components/AnalysisVerdict';
import { CustomDropdown } from '@/features/settings/components/CustomDropdown';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { type AnalysisResult, runAnalysis } from '@/lib/resume-ai';
import { useAIModels, useDocuments, useExtractText } from '@/services';
import { keys } from '@/services/query-client';
import {
  useAIModel,
  useOutputTone,
  usePreferencesStore,
  useResume,
} from '@/store/preferences-store';
import type { Model } from '@/types';

export const Route = createFileRoute('/analyze')({ component: Analyze });

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';
const MAX_BYTES = 25 * 1024 * 1024;

type Stage = 'idle' | 'running' | 'done';

function Analyze() {
  const { t, i18n } = useTranslation();
  const [resume, setResume] = useState('');
  const [jobAd, setJobAd] = useState('');
  const [stage, setStage] = useState<Stage>('idle');
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stream, setStream] = useState('');
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const [runId, setRunId] = useState(0);
  const abortControllerRef = useRef<AbortController | null>(null);
  const { data: modelList = [], isFetching: loadingModels } = useAIModels();
  const models = modelList as Model[];
  const qc = useQueryClient();
  const aiModel = useAIModel();
  const outputTone = useOutputTone();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const extractTextMutation = useExtractText();
  const resumePref = useResume();
  const { data: documentsRaw = [] } = useDocuments();

  // Auto-fill default resume on mount
  useEffect(() => {
    if (resume) return; // Don't override if user already has content
    const docs = documentsRaw as Array<DocumentRecord & { _id?: string; text?: string }>;
    const defaultDoc = docs.find((d) => (d._id ?? d.id) === resumePref?.defaultId) ?? docs[0];
    const text = defaultDoc?.text?.trim();
    if (text) setResume(text);
  }, [documentsRaw, resumePref?.defaultId, resume]);

  // Reset state when component unmounts (route change)
  useEffect(() => {
    return () => {
      setStage('idle');
      setResult(null);
      setError(null);
      setStream('');
    };
  }, []);

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

  const canRun = useMemo(
    () => resume.trim().length > 50 && jobAd.trim().length > 50 && !!aiModel?.defaultModel,
    [resume, jobAd, aiModel?.defaultModel]
  );

  const run = async () => {
    if (!canRun) return;
    setRunId((n) => n + 1);
    setStage('running');
    setError(null);
    setResult(null);
    setStream('');
    setCurrentJobId(null);

    // Create new abort controller for this run
    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      const analysis = await runAnalysis({
        resume,
        jobAd,
        model: aiModel?.defaultModel ?? '',
        locale: i18n.language,
        meta: { targetLocale: i18n.language, outputTone: outputTone ?? 'professional' },
        onToken: (tok) => setStream((p) => (p + tok).slice(-2000)),
        onJobId: (id) => setCurrentJobId(id),
        signal: controller.signal,
      });
      setResult(analysis);
      setStage('done');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('analyze.errorBody'));
      setStage('idle');
    } finally {
      setStream('');
      setCurrentJobId(null);
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
    setCurrentJobId(null);
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
                  <RotateCcw size={11} /> {t('analyze.reset')}
                </Button>
              )}
            </div>
            <p className="text-xs text-foreground/40">{t('analyze.subtitle')}</p>
          </div>

          {/* Model selector */}
          <div className="px-6 pb-4 flex items-center gap-2">
            <Button
              onClick={() => void qc.invalidateQueries({ queryKey: keys.ai.models })}
              disabled={loadingModels}
              className="flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.04] text-foreground/40 hover:text-foreground/70 transition-colors disabled:opacity-40 border-transparent p-0"
            >
              <RefreshCw size={11} className={loadingModels ? 'animate-spin' : ''} />
            </Button>
            <div className="flex-1">
              <CustomDropdown
                models={models}
                selectedModel={aiModel?.defaultModel ?? ''}
                onSelectModel={(n) =>
                  setAIModel({ defaultModel: n, temperature: 0.1, maxTokens: 3000 })
                }
              />
            </div>
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
              className={cn(
                'w-full justify-center',
                canRun && stage !== 'running' && 'hover:glow-purple'
              )}
            >
              {stage !== 'running' && <Sparkles size={14} />}
              {stage === 'running'
                ? t('analyze.running')
                : stage === 'done'
                  ? t('analyze.reAnalyse')
                  : !aiModel?.defaultModel
                    ? t('analyze.selectModel')
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
                  <AnalysisProgress running stream={stream} t={t} />
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

// ─── Collapsible input card (mirrors ai-generate FileInput) ──────────────────

function CollapsibleInput({
  label,
  icon: Icon,
  value,
  onChange,
  uploading,
  onUpload,
  placeholder,
  disabled,
  t,
}: {
  label: string;
  icon: React.ElementType;
  value: string;
  onChange: (v: string) => void;
  uploading: boolean;
  onUpload: (f: File) => void;
  placeholder: string;
  disabled?: boolean;
  t: (key: string) => string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const [expanded, setExpanded] = useState(true);

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl overflow-hidden transition-colors',
        value ? 'border-brand/20' : ''
      )}
    >
      <div className="flex items-center justify-between px-3 py-2.5">
        <div className="flex items-center gap-2">
          <Icon size={13} className={value ? 'text-brand-soft' : 'text-foreground/30'} />
          <span className="text-xs font-medium text-foreground/70">{label}</span>
          {value && <CheckCircle2 size={11} className="text-emerald-400" />}
        </div>
        <div className="flex items-center gap-2">
          {!disabled && (
            <>
              <input
                ref={ref}
                type="file"
                accept={ACCEPT_ATTR}
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) onUpload(f);
                  e.target.value = '';
                }}
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => ref.current?.click()}
                disabled={uploading || disabled}
                className="flex items-center gap-1 rounded-md bg-white/[0.04] px-2 py-1 text-[10px] text-foreground/50 hover:text-foreground/80 transition-colors h-auto"
              >
                <Upload size={10} className={uploading ? 'animate-pulse' : ''} />
                {uploading ? '…' : t('analyze.uploadButton')}
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setExpanded((e) => !e)}
            className="text-foreground/30 hover:text-foreground/60 transition-colors h-auto p-1"
          >
            <ChevronDown
              size={13}
              className={cn('transition-transform', expanded && 'rotate-180')}
            />
          </Button>
        </div>
      </div>
      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            initial={{ height: 0 }}
            animate={{ height: 'auto' }}
            exit={{ height: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <TextArea
              value={value}
              onChange={(e) => onChange(e.target.value)}
              disabled={disabled}
              placeholder={placeholder}
              className="w-full bg-transparent px-3 py-2.5 text-xs text-foreground/75 placeholder:text-foreground/35 font-mono leading-relaxed disabled:opacity-40"
              style={{ height: 140 }}
              spellCheck={false}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ─── Analysis progress ────────────────────────────────────────────────────────

// Estimated total duration in ms — progress bar reaches 90% at this point then waits
const ESTIMATED_MS = 50_000;

function AnalysisProgress({
  running,
  stream,
  t,
}: {
  running: boolean;
  stream: string;
  t: (key: string) => string;
}) {
  const PROGRESS_MESSAGES = [
    t('analyze.progress.reading'),
    t('analyze.progress.scanning'),
    t('analyze.progress.calculating'),
    t('analyze.progress.checking'),
    t('analyze.progress.measuring'),
    t('analyze.progress.identifying'),
    t('analyze.progress.prioritising'),
    t('analyze.progress.scoring'),
    t('analyze.progress.generating'),
    t('analyze.progress.risks'),
    t('analyze.progress.writing'),
    t('analyze.progress.finalising'),
  ];

  const [progress, setProgress] = useState(0); // 0–100
  const [elapsed, setElapsed] = useState(0); // seconds
  const [msgIdx, setMsgIdx] = useState(0);
  const startRef = useRef(Date.now());

  useEffect(() => {
    if (!running) return;

    startRef.current = Date.now();
    setProgress(0);
    setElapsed(0);
    setMsgIdx(0);

    const tick = setInterval(() => {
      const ms = Date.now() - startRef.current;
      setElapsed(Math.floor(ms / 1000));
      // Ease toward 90% over ESTIMATED_MS, then crawl slowly after
      const t = Math.min(ms / ESTIMATED_MS, 1);
      const eased = t < 1 ? 90 * (1 - Math.pow(1 - t, 3)) : 90 + (ms - ESTIMATED_MS) / 3000;
      setProgress(Math.min(eased, 99));
    }, 400);

    const msgTick = setInterval(() => {
      setMsgIdx((i) => (i + 1) % PROGRESS_MESSAGES.length);
    }, 3500);

    return () => {
      clearInterval(tick);
      clearInterval(msgTick);
    };
  }, [running, PROGRESS_MESSAGES.length]);

  const mins = Math.floor(elapsed / 60);
  const secs = elapsed % 60;
  const timer = mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
  const eta =
    elapsed > 5 && progress < 90
      ? `~${Math.max(1, Math.round(ESTIMATED_MS / 1000 - elapsed))}s left`
      : progress >= 90
        ? t('analyze.progress.almostDone')
        : '';

  return (
    <div className="mt-4 rounded-xl border border-white/[0.07] bg-white/[0.02] px-6 py-6 space-y-5">
      {/* Top row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-brand" />
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/35">
            {t('analyze.running')}
          </span>
        </div>
        <div className="flex items-center gap-3 text-[10px] text-foreground/30">
          {eta && <span className="text-brand-soft/70">{eta}</span>}
          <span>{timer}</span>
        </div>
      </div>

      {/* Rotating message */}
      <div className="relative h-6 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.p
            key={msgIdx}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={transition.slow}
            className="absolute inset-0 flex items-center text-sm text-foreground/60"
          >
            {PROGRESS_MESSAGES[msgIdx]}
          </motion.p>
        </AnimatePresence>
      </div>

      {/* Progress bar */}
      <div className="space-y-1.5">
        <div className="h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
          <motion.div
            className="h-full rounded-full bg-gradient-to-r from-violet-700 via-brand to-brand-soft"
            animate={{ width: `${progress}%` }}
            transition={transition.progress}
          />
        </div>
        <div className="flex justify-end text-[10px] text-foreground/20">
          <span>{Math.round(progress)}%</span>
        </div>
      </div>

      {/* Live stream output */}
      <div className="rounded-lg border border-white/[0.05] bg-black/20 px-4 py-3 h-28 overflow-hidden relative">
        {stream ? (
          <pre className="font-mono text-[10px] leading-relaxed text-foreground/30 whitespace-pre-wrap break-all">
            {stream.slice(-800)}
          </pre>
        ) : (
          <div className="space-y-2 pt-1">
            {[1, 0.7, 0.85, 0.5].map((w, i) => (
              <div
                key={i}
                className="h-2 rounded-full bg-white/[0.05] animate-pulse"
                style={{ width: `${w * 100}%`, animationDelay: `${i * 150}ms` }}
              />
            ))}
          </div>
        )}
        {/* Fade out top so it looks like it's scrolling in from below */}
        <div className="absolute inset-x-0 top-0 h-8 bg-gradient-to-b from-black/20 to-transparent pointer-events-none" />
      </div>
    </div>
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
