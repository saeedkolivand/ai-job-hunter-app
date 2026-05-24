import {
  AlertCircle,
  ArrowRight,
  Briefcase,
  Check,
  ChevronDown,
  RefreshCw,
  RotateCcw,
  Upload,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';

import { Button, TextArea } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { GenerationConfig } from '@/features/ai-generate/components/GenerationConfig';
import { GenerationMetadata } from '@/features/ai-generate/components/GenerationMetadata';
import { OutputPanelDone } from '@/features/ai-generate/components/OutputPanelDone';
import { OutputPanelExtracting } from '@/features/ai-generate/components/OutputPanelExtracting';
import { OutputPanelGenerating } from '@/features/ai-generate/components/OutputPanelGenerating';
import { OutputPanelIdle } from '@/features/ai-generate/components/OutputPanelIdle';
import { ResumeInputCard } from '@/features/ai-workspace/components/ResumeInputCard';
import { CustomDropdown } from '@/features/settings/components/CustomDropdown';
import { cn } from '@/lib/cn';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  type TemplateId,
} from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useAIModels, useExtractText } from '@/services';
import { keys } from '@/services/query-client';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

export const Route = createFileRoute('/ai-generate')({ component: AIGeneratePage });

const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';
const MAX_BYTES = 25 * 1024 * 1024;

type GenTarget = 'resume' | 'cover' | 'both';
type Stage =
  | 'idle'
  | 'extracting' // detecting metadata
  | 'configuring' // show detected info, pick mode
  | 'generating' // streaming
  | 'done';

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
  // Inputs
  const [resume, setResume] = useState('');
  const [jobAd, setJobAd] = useState('');
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);

  // Config
  const [mode, setMode] = useState<GenerationMode>('ats');
  const [target, setTarget] = useState<GenTarget>('both');
  const { data: modelList = [], isFetching: loadingModels } = useAIModels();
  const models = modelList as Model[];
  const qc = useQueryClient();
  const aiModel = useAIModel();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const extractTextMutation = useExtractText();

  // Stage
  const [stage, setStage] = useState<Stage>('idle');
  const [meta, setMeta] = useState<GenerationMeta | null>(null);
  const [stageLabel, setStageLabel] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Outputs
  const [resumeOut, setResumeOut] = useState('');
  const [coverOut, setCoverOut] = useState('');
  const [activeOut, setActiveOut] = useState<'resume' | 'cover'>('resume');
  const [templateId, setTemplateId] = useState<TemplateId>('modern');

  // Streaming preview
  const [streamBuffer, setStreamBuffer] = useState('');
  const stageIdxRef = useRef(0);
  const stageTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  // Copy state
  const [copied, setCopied] = useState(false);

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
  const canGenerate = canProceed && !!aiModel?.defaultModel;

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
      const detected = await extractMetadata(resume, jobAd, aiModel?.defaultModel ?? '');
      setMeta(detected);
      setStage('configuring');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('aiGenerate.errors.extractionFailed'));
      setStage('idle');
    }
  };

  // Step 2 — generate
  const handleGenerate = async () => {
    if (!meta || !aiModel?.defaultModel) return;
    setError(null);
    setResumeOut('');
    setCoverOut('');
    setStreamBuffer('');
    setStage('generating');
    startStageRotation();

    // Create abort controller for this generation
    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      if (target === 'resume' || target === 'both') {
        setActiveOut('resume');
        setStreamBuffer('');
        await generateResume(
          resume,
          jobAd,
          meta,
          mode,
          aiModel.defaultModel,
          (tok) => {
            setResumeOut((p) => p + tok);
            setStreamBuffer((p) => (p + tok).slice(-600));
          },
          undefined,
          controller.signal
        );
      }

      if (target === 'cover' || target === 'both') {
        setActiveOut('cover');
        setStreamBuffer('');
        await generateCoverLetter(
          resume,
          jobAd,
          meta,
          mode,
          aiModel.defaultModel,
          (tok) => {
            setCoverOut((p) => p + tok);
            setStreamBuffer((p) => (p + tok).slice(-600));
          },
          undefined,
          controller.signal
        );
      }

      stopStageRotation();
      setStreamBuffer('');
      setStage('done');
      setActiveOut(target === 'cover' ? 'cover' : 'resume');
    } catch (err) {
      stopStageRotation();
      setError(err instanceof Error ? err.message : t('aiGenerate.errors.generationFailed'));
      setStage('configuring');
    } finally {
      abortControllerRef.current = null;
    }
  };

  const reset = () => {
    // Abort running generation if exists
    if (abortControllerRef.current && stage === 'generating') {
      abortControllerRef.current.abort();
    }
    stopStageRotation();
    setStage('idle');
    setMeta(null);
    setError(null);
    setResumeOut('');
    setCoverOut('');
    setStreamBuffer('');
    setResume('');
    setJobAd('');
  };

  const copyOutput = async () => {
    const text = activeOut === 'resume' ? resumeOut : coverOut;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const currentOutput = activeOut === 'resume' ? resumeOut : coverOut;

  // ── Export ──
  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
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
      await exportPDF(text, name, type, meta ?? undefined, templateId);
    }
    if (fmt === 'docx') {
      await exportDOCX(text, name, type, meta ?? undefined, templateId);
    }
    if (fmt === 'txt') {
      exportTXT(text, name);
    }
  };

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
                  <RotateCcw size={11} /> {t('aiGenerate.regenerate')}
                </Button>
              )}
            </div>
            <p className="text-xs text-foreground/40">{t('aiGenerate.subtitle')}</p>
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
                  setAIModel({ defaultModel: n, temperature: 0.25, maxTokens: 3000 })
                }
              />
            </div>
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
            onModeChange={setMode}
            onTargetChange={setTarget}
            onTemplateChange={setTemplateId}
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
                className={cn('w-full justify-center', canGenerate && 'hover:glow-purple')}
              >
                <ArrowRight size={14} />
                {!aiModel?.defaultModel
                  ? t('aiGenerate.selectModel')
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
            {stage === 'idle' && <OutputPanelIdle />}

            {/* Extracting */}
            {stage === 'extracting' && <OutputPanelExtracting stageLabel={stageLabel} />}

            {/* Generating */}
            {stage === 'generating' && (
              <OutputPanelGenerating
                stageLabel={stageLabel}
                streamBuffer={streamBuffer}
                activeOut={activeOut}
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
                _templateId={templateId}
                onActiveOutChange={setActiveOut}
                onCopy={() => void copyOutput()}
                onExport={doExport}
                onOutputChange={(value) => {
                  if (activeOut === 'resume') setResumeOut(value);
                  else setCoverOut(value);
                }}
                onRegenerate={() => {
                  setStage('configuring');
                  setResumeOut('');
                  setCoverOut('');
                }}
                copied={copied}
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
