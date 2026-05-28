import { Check, Copy, Download, ExternalLink, Loader2, Sparkles, Wand2, X } from 'lucide-react';
import { useRef, useState } from 'react';

import type { AutopilotFoundJob } from '@ajh/shared';
import { Button, cn, ModalShell } from '@ajh/ui';

import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import { ModelSelector, useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
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
import { useExtractText, useOpenExternal, useResolveJobUrl } from '@/services';

type Target = 'resume' | 'cover' | 'both';
const TEMPLATE: TemplateId = 'modern';
const MODE: GenerationMode = 'ats';

interface Props {
  job: AutopilotFoundJob;
  /** The resume the autopilot used — pre-fills the resume input. */
  resumeText?: string;
  onClose: () => void;
}

/**
 * Tailor a resume / cover letter for a single autopilot-found job, inline.
 * Reuses the AI Generate primitives so the user never leaves the Autopilot page.
 */
export function ApplyJobModal({ job, resumeText, onClose }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();
  const extractTextMutation = useExtractText();
  const openExternal = useOpenExternal();

  const [resume, setResume] = useState(resumeText ?? '');
  const [target, setTarget] = useState<Target>('cover');
  const [generating, setGenerating] = useState(false);
  const [phase, setPhase] = useState<'idle' | 'analyzing' | 'resume' | 'cover'>('idle');
  const [resumeOut, setResumeOut] = useState('');
  const [coverOut, setCoverOut] = useState('');
  const [activeOut, setActiveOut] = useState<'resume' | 'cover'>('cover');
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [meta, setMeta] = useState<GenerationMeta | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  // Fetch the description on demand when the board's list scrape omitted it.
  const initialDesc = (job.description ?? '').trim();
  const resolved = useResolveJobUrl(job.url, !initialDesc);
  const fetchedDesc = (resolved.data?.description ?? '').trim();
  const jobDesc = initialDesc || fetchedDesc;
  const hasDesc = jobDesc.length > 0;
  const fetchingDesc = !initialDesc && resolved.isLoading;
  const output = activeOut === 'resume' ? resumeOut : coverOut;

  const close = () => {
    abortRef.current?.abort();
    onClose();
  };

  const handleUpload = async (file: File) => {
    setUploading(true);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const res = (await extractTextMutation.mutateAsync({
        name: file.name,
        bytes,
      })) as { text: string };
      const text = (res?.text ?? '').trim();
      if (text) setResume(text);
    } finally {
      setUploading(false);
    }
  };

  const handleGenerate = async () => {
    if (!canUse || !hasDesc || generating || !resume.trim()) return;
    setError(null);
    setGenerating(true);
    setPhase('analyzing');
    setResumeOut('');
    setCoverOut('');
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      const detected = await extractMetadata(resume, jobDesc, model);
      setMeta(detected);
      if (target === 'resume' || target === 'both') {
        setActiveOut('resume');
        setPhase('resume');
        const r = await generateResume(
          resume,
          jobDesc,
          detected,
          MODE,
          model,
          (tok) => setResumeOut((p) => p + tok),
          'en',
          controller.signal
        );
        setResumeOut(r);
      }
      if (target === 'cover' || target === 'both') {
        setActiveOut('cover');
        setPhase('cover');
        const c = await generateCoverLetter(
          resume,
          jobDesc,
          detected,
          MODE,
          model,
          (tok) => setCoverOut((p) => p + tok),
          'en',
          controller.signal
        );
        setCoverOut(c);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('autopilot.apply.failed'));
    } finally {
      setGenerating(false);
      setPhase('idle');
      abortRef.current = null;
    }
  };

  const phaseLabel =
    phase === 'analyzing'
      ? t('autopilot.apply.analyzing')
      : phase === 'resume'
        ? t('autopilot.apply.writingResume')
        : phase === 'cover'
          ? t('autopilot.apply.writingCover')
          : '';

  const handleCopy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const handleExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    setExportOpen(false);
    if (!output) return;
    const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const fileMeta: GenerationMeta = meta ?? {
      candidateName: '',
      jobTitle: '',
      companyName: '',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    };
    const name = buildFilename(fileMeta, docType, fmt);
    if (fmt === 'pdf') await exportPDF(output, name, docType, meta ?? undefined, TEMPLATE, false);
    else if (fmt === 'docx')
      await exportDOCX(output, name, docType, meta ?? undefined, TEMPLATE, false);
    else exportTXT(output, name);
  };

  const targets: { id: Target; label: string }[] = [
    { id: 'cover', label: t('autopilot.apply.target.cover') },
    { id: 'resume', label: t('autopilot.apply.target.resume') },
    { id: 'both', label: t('autopilot.apply.target.both') },
  ];

  return (
    <ModalShell open onClose={close} maxWidth="max-w-2xl">
      <div className="flex max-h-[85vh] flex-col">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Wand2 size={14} className="shrink-0 text-brand-soft" />
              <span className="truncate text-sm font-semibold text-foreground/85">{job.title}</span>
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {job.company}
              {job.location ? ` · ${job.location}` : ''}
            </div>
          </div>
          <Button
            onClick={close}
            className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
          >
            <X size={16} />
          </Button>
        </div>

        {/* Body */}
        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
          {/* Job description */}
          <div>
            <div className="mb-1 text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/35">
              {t('autopilot.apply.jobDescription')}
            </div>
            {fetchingDesc ? (
              <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] text-foreground/40">
                <Loader2 size={12} className="animate-spin" />
                {t('autopilot.apply.fetchingDescription')}
              </div>
            ) : hasDesc ? (
              <div className="max-h-32 overflow-y-auto whitespace-pre-wrap rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
                {jobDesc}
              </div>
            ) : job.url ? (
              <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
                {t('autopilot.apply.loadFailed')}{' '}
                <button
                  type="button"
                  onClick={() => void openExternal.mutate(job.url)}
                  className="inline-flex items-center gap-0.5 font-medium text-brand-soft hover:underline"
                >
                  {t('autopilot.viewJob')}
                  <ExternalLink size={10} />
                </button>
              </div>
            ) : (
              <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
                {t('autopilot.apply.noDescription')}
              </div>
            )}
          </div>

          {/* Resume input */}
          <ResumeInputCard
            value={resume}
            onChange={setResume}
            onUpload={handleUpload}
            uploading={uploading}
          />

          {/* Model */}
          <ModelSelector />

          {/* Target + generate */}
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-1 rounded-lg bg-white/[0.04] p-0.5">
              {targets.map(({ id, label }) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setTarget(id)}
                  className={cn(
                    'rounded-md px-3 py-1 text-[11px] font-medium transition-all',
                    target === id
                      ? 'bg-white/[0.08] text-foreground/90 shadow-sm'
                      : 'text-foreground/40 hover:text-foreground/60'
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
            <Button
              variant="glass"
              size="sm"
              loading={generating}
              disabled={!canUse || !hasDesc || generating || !resume.trim()}
              onClick={() => void handleGenerate()}
            >
              {!generating && <Sparkles size={13} />}
              {generating ? t('autopilot.apply.generating') : t('autopilot.apply.generate')}
            </Button>
          </div>

          {!canUse && (
            <p className="text-[11px] text-amber-300/70">
              {reason === 'addApiKey'
                ? t('autopilot.apply.addApiKey')
                : t('autopilot.apply.selectModel')}
            </p>
          )}
          {error && <p className="text-[11px] text-red-300/80">{error}</p>}

          {/* Generation progress */}
          {generating && (
            <div className="flex items-center gap-2 rounded-lg border border-brand/20 bg-brand/5 px-3 py-2 text-[11px] text-brand-soft">
              <Loader2 size={12} className="animate-spin" />
              {phaseLabel}
              {target === 'both' && phase !== 'analyzing' && (
                <span className="text-foreground/40">· {phase === 'resume' ? '1/2' : '2/2'}</span>
              )}
            </div>
          )}

          {/* Output */}
          {(resumeOut || coverOut) && (
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.02]">
              <div className="flex items-center justify-between border-b border-white/[0.06] px-3 py-2">
                {target === 'both' ? (
                  <div className="flex items-center gap-1">
                    {(['resume', 'cover'] as const).map((o) => (
                      <button
                        key={o}
                        type="button"
                        onClick={() => setActiveOut(o)}
                        className={cn(
                          'rounded px-2 py-0.5 text-[10px] font-medium transition-colors',
                          activeOut === o
                            ? 'bg-brand/15 text-brand-soft'
                            : 'text-foreground/40 hover:text-foreground/70'
                        )}
                      >
                        {o === 'resume'
                          ? t('autopilot.apply.target.resume')
                          : t('autopilot.apply.target.cover')}
                      </button>
                    ))}
                  </div>
                ) : (
                  <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/35">
                    {activeOut === 'resume'
                      ? t('autopilot.apply.target.resume')
                      : t('autopilot.apply.target.cover')}
                  </span>
                )}
                <div className="flex items-center gap-3">
                  <Button
                    onClick={() => void handleCopy()}
                    disabled={!output}
                    className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-foreground/40 hover:text-foreground/70"
                  >
                    {copied ? <Check size={11} /> : <Copy size={11} />}
                    {copied ? t('autopilot.apply.copied') : t('autopilot.apply.copy')}
                  </Button>
                  <div className="relative">
                    <Button
                      onClick={() => setExportOpen((o) => !o)}
                      disabled={!output}
                      className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-brand-soft hover:text-brand-soft/80"
                    >
                      <Download size={11} />
                      {t('aiGenerate.export')}
                    </Button>
                    {exportOpen && (
                      <>
                        <div
                          className="fixed inset-0 z-[650]"
                          onClick={() => setExportOpen(false)}
                        />
                        <div className="absolute right-0 top-full z-[700] mt-1.5 w-32 overflow-hidden rounded-lg border border-white/10 bg-secondary shadow-2xl">
                          {(['pdf', 'docx', 'txt'] as const).map((fmt) => (
                            <button
                              key={fmt}
                              type="button"
                              onClick={() => void handleExport(fmt)}
                              className="flex w-full items-center gap-2 px-3 py-2 text-[11px] text-foreground/65 transition-colors hover:bg-white/[0.05] hover:text-foreground"
                            >
                              <Download size={10} />
                              {t('aiGenerate.download', { fmt: fmt.toUpperCase() })}
                            </button>
                          ))}
                        </div>
                      </>
                    )}
                  </div>
                </div>
              </div>
              <div className="max-h-56 overflow-y-auto whitespace-pre-wrap px-3 py-2 text-[11px] leading-relaxed text-foreground/75">
                {output || '…'}
              </div>
            </div>
          )}
        </div>
      </div>
    </ModalShell>
  );
}
