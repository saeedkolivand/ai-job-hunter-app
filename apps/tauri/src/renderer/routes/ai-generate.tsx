import {
  AlertCircle,
  ArrowRight,
  Briefcase,
  Check,
  ChevronDown,
  RefreshCw,
  Upload,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button, TextArea } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { ModelSelector, useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { GenerationConfig } from '@/features/ai-generate/components/GenerationConfig';
import { GenerationMetadata } from '@/features/ai-generate/components/GenerationMetadata';
import { OutputPanelDone } from '@/features/ai-generate/components/OutputPanelDone';
import { OutputPanelExtracting } from '@/features/ai-generate/components/OutputPanelExtracting';
import { OutputPanelGenerating } from '@/features/ai-generate/components/OutputPanelGenerating';
import { OutputPanelIdle } from '@/features/ai-generate/components/OutputPanelIdle';
import { ResumeInputCard } from '@/features/ai-workspace/components/ResumeInputCard';
import { cn } from '@/lib/cn';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMode,
  type TemplateId,
} from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useExtractText } from '@/services';
import { useSaveAiGeneration } from '@/services/use-ai-generations';
import { useSessionStore } from '@/store/session-store';

export const Route = createFileRoute('/ai-generate')({ component: AIGeneratePage });

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';
const MAX_BYTES = 25 * 1024 * 1024;

type GenTarget = 'resume' | 'cover' | 'both';

const GENERATION_STAGES = [
  'aiGenerate.stages.analyzing',
  'aiGenerate.stages.extracting',
  'aiGenerate.stages.mapping',
  'aiGenerate.stages.optimizing',
  'aiGenerate.stages.rewriting',
  'aiGenerate.stages.adapting',
  'aiGenerate.stages.finalizing',
];

function AIGeneratePage() {
  const { t } = useTranslation();

  // Persistent state (survives navigation)
  const { aiGenerate, setAIGenerate, resetAIGenerate } = useSessionStore();
  const {
    resume,
    jobAd,
    stage,
    meta,
    mode,
    target,
    templateId,
    atsMode,
    resumeOut,
    coverOut,
    activeOut,
  } = aiGenerate;

  const setResume = (v: string) => setAIGenerate({ resume: v });
  const setJobAd = (v: string) => setAIGenerate({ jobAd: v });
  const setStage = (v: typeof stage) => setAIGenerate({ stage: v });
  const setMeta = (v: typeof meta) => setAIGenerate({ meta: v });
  const setMode = (v: GenerationMode) => setAIGenerate({ mode: v });
  const setTarget = (v: GenTarget) => setAIGenerate({ target: v });
  const setTemplateId = (v: TemplateId) => setAIGenerate({ templateId: v });
  const setAtsMode = (v: boolean) => setAIGenerate({ atsMode: v });
  const setResumeOut = (v: string | ((p: string) => string)) =>
    setAIGenerate({ resumeOut: typeof v === 'function' ? v(resumeOut) : v });
  const setCoverOut = (v: string | ((p: string) => string)) =>
    setAIGenerate({ coverOut: typeof v === 'function' ? v(coverOut) : v });
  const setActiveOut = (v: 'resume' | 'cover') => setAIGenerate({ activeOut: v });

  // Transient state (resets each run)
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const [stageLabel, setStageLabel] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [streamBuffer, setStreamBuffer] = useState('');
  const [thinkingBuffer, setThinkingBuffer] = useState('');
  const [copied, setCopied] = useState(false);
  const [modelLoading, setModelLoading] = useState(false);
  const [tokenCount, setTokenCount] = useState(0);
  const tokenStartRef = useRef<number | null>(null);
  const [genStep, setGenStep] = useState<{ current: number; total: number; label: string } | null>(
    null
  );

  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  const extractTextMutation = useExtractText();

  const stageIdxRef = useRef(0);
  const stageTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  const handleUpload = async (target: 'resume' | 'jobAd', file: File) => {
    setUploadError(null);
    const ext = file.name.toLowerCase().split('.').pop() ?? '';
    if (!ACCEPTED_EXTS.includes(ext as (typeof ACCEPTED_EXTS)[number])) {
      setUploadError(t('aiGenerate.errors.unsupportedFileType', { ext }));
      return;
    }
    if (file.size > MAX_BYTES) {
      setUploadError(t('aiGenerate.errors.fileTooLarge'));
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
        setUploadError(t('aiGenerate.errors.couldNotExtract'));
        return;
      }
      if (target === 'resume') setResume(text);
      else setJobAd(text);
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : t('aiGenerate.errors.uploadFailed'));
    } finally {
      setUploading(null);
    }
  };

  const canProceed = resume.trim().length > 50 && jobAd.trim().length > 50;
  const canGenerate = canProceed && canUseAI;

  const startStageRotation = () => {
    stageIdxRef.current = 0;
    setStageLabel(t(GENERATION_STAGES[0] ?? ''));
    stageTimerRef.current = setInterval(() => {
      stageIdxRef.current = (stageIdxRef.current + 1) % GENERATION_STAGES.length;
      setStageLabel(t(GENERATION_STAGES[stageIdxRef.current] ?? ''));
    }, 2800);
  };

  const stopStageRotation = () => {
    if (stageTimerRef.current) {
      clearInterval(stageTimerRef.current);
      stageTimerRef.current = null;
    }
  };

  // Step 1 — pre-process
  const handleAnalyze = async () => {
    if (!canGenerate) return;
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

  // Step 2 — generate
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

    // Create abort controller for this generation
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
        await generateResume(
          resume,
          jobAd,
          meta,
          mode,
          selectedModel,
          onTok(setStreamBuffer, (t) => {
            finalResume += t;
            setResumeOut((p) => p + t);
          }),
          undefined,
          controller.signal,
          onThink
        );
      }

      if (target === 'cover' || target === 'both') {
        setActiveOut('cover');
        setStreamBuffer('');
        setThinkingBuffer('');
        setModelLoading(true);
        tokenStartRef.current = null;
        setTokenCount(0);
        setGenStep({ current: 2, total: 2, label: 'Cover Letter' });
        await generateCoverLetter(
          resume,
          jobAd,
          meta,
          mode,
          selectedModel,
          onTok(setStreamBuffer, (t) => {
            finalCover += t;
            setCoverOut((p) => p + t);
          }),
          undefined,
          controller.signal,
          onThink
        );
      }

      stopStageRotation();
      setStreamBuffer('');
      setStage('done');
      // Pick the first tab that actually has content so the textarea is never blank
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

  const reset = () => {
    if (abortControllerRef.current && stage === 'generating') {
      abortControllerRef.current.abort();
    }
    stopStageRotation();
    setError(null);
    setStreamBuffer('');
    setThinkingBuffer('');
    resetAIGenerate();
  };

  const copyOutput = async () => {
    if (isGenerating) return;
    const text = activeOut === 'resume' ? resumeOut : coverOut;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const currentOutput = activeOut === 'resume' ? resumeOut : coverOut;

  // ── Export ──
  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    if (isGenerating) return;
    const text = currentOutput;
    if (!text) return;
    const type = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const name = buildFilename(
      meta ?? {
        candidateName: '',
        jobTitle: '',
        companyName: '',
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        targetLanguage: 'en',
        topRequirements: [],
      },
      type,
      fmt
    );
    if (fmt === 'pdf') {
      await exportPDF(text, name, type, meta ?? undefined, templateId, atsMode);
    }
    if (fmt === 'docx') {
      await exportDOCX(text, name, type, meta ?? undefined, templateId, atsMode);
    }
    if (fmt === 'txt') {
      exportTXT(text, name);
    }
  };

  const saveAiGeneration = useSaveAiGeneration();

  const isGenerating = stage === 'generating';

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full">
        {/* ── LEFT PANEL — Inputs + config ─────────────────────────────── */}
        <div className="flex w-[420px] shrink-0 flex-col border-r border-white/[0.05] overflow-y-auto">
          {/* Header */}
          <div className="px-6 pt-8 pb-4">
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2">
                <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-brand/15">
                  <Wand2 size={14} className="text-brand-soft" />
                </div>
                <span className="text-base font-semibold text-foreground/90">
                  {t('aiGenerate.title')}
                </span>
              </div>
              {(stage === 'configuring' || stage === 'done') && (
                <Button
                  onClick={reset}
                  className="flex items-center gap-1 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
                >
                  <RefreshCw size={11} /> {t('aiGenerate.regenerate')}
                </Button>
              )}
            </div>
            <p className="text-xs text-foreground/40">{t('aiGenerate.subtitle')}</p>
          </div>

          {/* Model selector */}
          <div className="px-6 pb-4">
            <ModelSelector />
          </div>

          <div className="px-6 space-y-3 pb-4">
            {/* Resume input */}
            <ResumeInputCard
              value={resume}
              onChange={setResume}
              onUpload={(f) => handleUpload('resume', f)}
              uploading={uploading === 'resume'}
              disabled={stage !== 'idle'}
              placeholder={t('aiGenerate.resumePlaceholder')}
            />

            {/* Job ad input */}
            <FileInput
              label={t('aiGenerate.jobAdLabel')}
              icon={Briefcase}
              value={jobAd}
              onChange={setJobAd}
              uploading={uploading === 'jobAd'}
              onUpload={(f) => void handleUpload('jobAd', f)}
              disabled={stage !== 'idle'}
              t={t}
            />

            {uploadError && (
              <div className="flex items-center gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-xs text-amber-200/80">
                <AlertCircle size={11} /> {uploadError}
              </div>
            )}
          </div>

          {/* Detected metadata — shown after extraction */}
          <GenerationMetadata meta={meta} />

          {/* Config — target + mode */}
          <GenerationConfig
            stage={stage}
            mode={mode}
            target={target}
            templateId={templateId}
            atsMode={atsMode}
            onModeChange={setMode}
            onTargetChange={setTarget}
            onTemplateChange={(id) => {
              setTemplateId(id);
              if (id !== 'two-column') setAtsMode(false);
            }}
            onAtsModeChange={setAtsMode}
            onGenerate={() => void handleGenerate()}
            isGenerating={isGenerating}
          />

          {/* Idle CTA */}
          {stage === 'idle' && (
            <div className="px-6 pb-6 mt-auto">
              <Button
                size="md"
                variant={canGenerate ? 'glass' : 'ghost'}
                onClick={() => void handleAnalyze()}
                disabled={!canGenerate}
                className="w-full justify-center transition-all duration-150 ease-out"
              >
                <ArrowRight size={14} />
                {!canUseAI
                  ? aiReason === 'addApiKey'
                    ? t('aiGenerate.addApiKey')
                    : t('aiGenerate.selectModel')
                  : !canProceed
                    ? t('aiGenerate.pasteResumeJob')
                    : t('aiGenerate.continue')}
              </Button>
            </div>
          )}
        </div>

        {/* ── RIGHT PANEL — Output ──────────────────────────────────────── */}
        <div className="flex flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {/* Idle state */}
            {(stage === 'idle' || stage === 'configuring') && <OutputPanelIdle />}

            {/* Extracting */}
            {stage === 'extracting' && <OutputPanelExtracting stageLabel={stageLabel} />}

            {/* Generating */}
            {stage === 'generating' && (
              <OutputPanelGenerating
                stageLabel={stageLabel}
                streamBuffer={streamBuffer}
                activeOut={activeOut}
                thinkingBuffer={thinkingBuffer}
                modelLoading={modelLoading}
                genStep={genStep}
                tokenCount={tokenCount}
                tokenStartMs={tokenStartRef.current}
              />
            )}

            {/* Done — output */}
            {stage === 'done' && (
              <OutputPanelDone
                resumeOut={resumeOut}
                coverOut={coverOut}
                activeOut={activeOut}
                meta={meta}
                mode={mode}
                templateId={templateId}
                onActiveOutChange={setActiveOut}
                onCopy={() => void copyOutput()}
                onExport={doExport}
                onOutputChange={(value) => {
                  if (activeOut === 'resume') setResumeOut(value);
                  else setCoverOut(value);
                }}
                onRegenerate={() => void handleGenerate()}
                copied={copied}
                isGenerating={isGenerating}
              />
            )}
          </AnimatePresence>

          {/* Error */}
          {error && (
            <div className="shrink-0 mx-6 mb-4 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-3 text-xs text-red-300/80">
              <div className="font-medium mb-0.5">{t('aiGenerate.error')}</div>
              {error}
            </div>
          )}
        </div>
      </div>
    </PageTransition>
  );
}

// ─── File input card ──────────────────────────────────────────────────────────

interface FileInputProps {
  label: string;
  icon: React.ElementType;
  value: string;
  onChange: (v: string) => void;
  uploading: boolean;
  onUpload: (file: File) => void;
  disabled?: boolean;
  t: (key: string) => string;
}

function FileInput({
  label,
  icon: Icon,
  value,
  onChange,
  uploading,
  onUpload,
  disabled,
  t,
}: FileInputProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [expanded, setExpanded] = useState(true);

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl overflow-hidden transition-colors',
        value ? 'border-brand/20' : ''
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2.5">
        <div className="flex items-center gap-2">
          <Icon size={13} className={value ? 'text-brand-soft' : 'text-foreground/30'} />
          <span className="text-xs font-medium text-foreground/70">{label}</span>
          {value && <Check size={11} className="text-emerald-400" />}
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
                {uploading ? '…' : t('aiGenerate.upload')}
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

      {/* Textarea */}
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
              placeholder={t('aiGenerate.placeholder').replace('…', '')}
              className="w-full bg-transparent px-3 py-2.5 text-xs text-foreground/75 placeholder:text-foreground/35 font-mono leading-relaxed disabled:opacity-40"
              style={{ height: '140px' }}
              spellCheck={false}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ─── File input card ──────────────────────────────────────────────────────────
