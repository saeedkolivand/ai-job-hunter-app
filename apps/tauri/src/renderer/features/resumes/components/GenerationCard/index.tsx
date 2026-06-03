import {
  Building2,
  Calendar,
  Check,
  ChevronDown,
  Copy,
  Download,
  ExternalLink as ExternalLinkIcon,
  FileText,
  HelpCircle,
  Loader2,
  Search,
  Trash2,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { Button, cn, ConfirmModal, GlassCard, SegmentedControl, transition } from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import { ExternalLink } from '@/components/ui/ExternalLink';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  PERSIST_DEBOUNCE_MS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useRemoveAiGeneration, useUpdateAiGeneration } from '@/services/use-ai-generations';

const EXPORT_FORMATS = ['pdf', 'docx', 'txt'] as const;
type ExportFormat = (typeof EXPORT_FORMATS)[number];

const TEMPLATE_OPTIONS: { id: TemplateId; label: string }[] = Object.values(TEMPLATES).map((t) => ({
  id: t.id,
  label: t.name,
}));

interface GenerationCardProps {
  gen: AiGenerationRecord;
}

export function GenerationCard({ gen }: GenerationCardProps) {
  const { t } = useTranslation();
  const removeAiGeneration = useRemoveAiGeneration();
  const updateAiGeneration = useUpdateAiGeneration();
  const [expanded, setExpanded] = useState<
    'resume' | 'cover' | 'jobAd' | 'brief' | 'answers' | null
  >(null);
  const [copied, setCopied] = useState<'resume' | 'cover' | null>(null);
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');
  const [exportTemplate, setExportTemplate] = useState<TemplateId>('modern');
  const [exporting, setExporting] = useState<'resume' | 'cover' | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Local editing buffers keep typing smooth and own the edit truth for the card's
  // lifetime. The card is keyed by `gen.id` at the list (it remounts per record),
  // so drafts are seeded once from the record on mount; thereafter the optimistic
  // update hook patches the list cache (with rollback on failure) and we do NOT
  // re-sync the drafts from `gen`. A re-sync here would let the post-`onSettled`
  // refetch overwrite the buffer with debounce-stale text, clobbering keystrokes
  // typed during the 800ms debounce window.
  const [resumeDraft, setResumeDraft] = useState(gen.resumeText);
  const [coverDraft, setCoverDraft] = useState(gen.coverLetterText);

  // Debounced persistence — one timer per field; flushed on unmount.
  const persistTimers = useRef<{
    resume?: ReturnType<typeof setTimeout>;
    cover?: ReturnType<typeof setTimeout>;
  }>({});
  useEffect(() => {
    const timers = persistTimers.current;
    return () => {
      if (timers.resume) clearTimeout(timers.resume);
      if (timers.cover) clearTimeout(timers.cover);
    };
  }, []);

  const persistEdit = (type: 'resume' | 'cover', text: string) => {
    const existing = persistTimers.current[type];
    if (existing) clearTimeout(existing);
    persistTimers.current[type] = setTimeout(() => {
      updateAiGeneration.mutate(
        type === 'resume' ? { id: gen.id, resumeText: text } : { id: gen.id, coverLetterText: text }
      );
    }, PERSIST_DEBOUNCE_MS);
  };

  const onEdit = (type: 'resume' | 'cover', text: string) => {
    if (type === 'resume') setResumeDraft(text);
    else setCoverDraft(text);
    persistEdit(type, text);
  };

  const handleDelete = () => {
    setConfirmDelete(false);
    removeAiGeneration.mutate(gen.id);
  };

  const copy = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? resumeDraft : coverDraft;
    await navigator.clipboard.writeText(text);
    setCopied(type);
    setTimeout(() => setCopied(null), 1800);
  };

  const meta = {
    candidateName: gen.candidateName,
    jobTitle: gen.jobTitle,
    companyName: gen.companyName,
    resumeLanguage: gen.resumeLanguage,
    jobAdLanguage: gen.jobAdLanguage,
    targetLanguage: gen.targetLanguage,
    topRequirements: gen.topRequirements,
    mismatch: gen.mismatch,
  };

  const doExport = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? resumeDraft : coverDraft;
    if (!text) return;
    const docType = type === 'resume' ? 'resume' : 'cover-letter';
    const filename = buildFilename(meta, docType, exportFormat);
    setExporting(type);
    try {
      if (exportFormat === 'pdf') {
        await exportPDF(text, filename.replace('.pdf', ''), docType, meta, exportTemplate);
      } else if (exportFormat === 'docx') {
        await exportDOCX(text, filename.replace('.docx', ''), docType, meta, exportTemplate);
      } else {
        exportTXT(text, filename.replace('.txt', ''));
      }
    } finally {
      setExporting(null);
    }
  };

  const generatedDate = new Date(gen.createdAt).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });

  const formatRelative = (ts: number): string => {
    const diff = Date.now() - ts;
    const m = Math.floor(diff / 60000);
    const h = Math.floor(m / 60);
    const d = Math.floor(h / 24);
    if (m < 1) return t('resumes.relativeTime.justNow');
    if (m < 60) return t('resumes.relativeTime.minutesAgo', { m });
    if (h < 24) return t('resumes.relativeTime.hoursAgo', { h });
    if (d < 7) return t('resumes.relativeTime.daysAgo', { d });
    return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  };

  return (
    <>
      <GlassCard tone="graphite" className="rounded-xl overflow-hidden p-0">
        {/* Header */}
        <div className="flex items-center gap-4 p-4">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand/10">
            <Wand2 size={14} className="text-brand-soft" />
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground/90">
              <span className="truncate">{gen.jobTitle || t('resumes.unknownPosition')}</span>
              {gen.companyName && (
                <span className="text-foreground/40 text-xs font-normal shrink-0">
                  @ {gen.companyName}
                </span>
              )}
            </div>
            <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-foreground/50">
              {gen.candidateName && <span>{gen.candidateName}</span>}
              <span className="flex items-center gap-1 text-foreground/35">
                <Calendar size={10} />
                <span title={formatRelative(gen.createdAt)}>{generatedDate}</span>
              </span>
              <span className="rounded-full border border-brand/20 bg-brand/8 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                {gen.mode}
              </span>
              {gen.board && (
                <span className="rounded-full border border-white/[0.06] bg-white/[0.03] px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-foreground/55">
                  {gen.board}
                </span>
              )}
              {/* A linked job means this generation was an application — surface the
                "Applied" state and a link back to the original posting. */}
              {gen.jobUrl && (
                <span className="flex items-center gap-0.5 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-emerald-300">
                  <Check size={9} /> {t('resumes.generated.applied')}
                </span>
              )}
              {gen.jobUrl && (
                <ExternalLink
                  href={gen.jobUrl}
                  title={t('resumes.generated.openPosting')}
                  className="flex items-center gap-1 text-foreground/35 transition-colors hover:text-brand-soft"
                >
                  <ExternalLinkIcon size={10} /> {t('resumes.generated.openPosting')}
                </ExternalLink>
              )}
            </div>
          </div>

          <div className="flex items-center gap-1 shrink-0">
            {resumeDraft && (
              <Button
                onClick={() => void copy('resume')}
                className="flex items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
              >
                <Copy size={10} />
                {copied === 'resume'
                  ? t('resumes.generated.copied')
                  : t('resumes.generated.copyResume')}
              </Button>
            )}
            {coverDraft && (
              <Button
                onClick={() => void copy('cover')}
                className="flex items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
              >
                <Copy size={10} />
                {copied === 'cover'
                  ? t('resumes.generated.copied')
                  : t('resumes.generated.copyCoverLetter')}
              </Button>
            )}
            <Button
              onClick={() => setConfirmDelete(true)}
              aria-label={t('resumes.generated.delete')}
              title={t('resumes.generated.delete')}
              className="flex items-center gap-1 rounded-lg bg-white/5 px-2 py-1.5 text-[11px] text-foreground/40 transition-colors hover:text-red-400 h-auto border-transparent"
            >
              <Trash2 size={10} />
            </Button>
          </div>
        </div>

        {/* Export bar */}
        {(resumeDraft || coverDraft) && (
          <div className="border-t border-white/[0.04] px-4 py-2.5 flex items-center gap-2 flex-wrap">
            <Download size={11} className="text-foreground/30 shrink-0" />
            <span className="text-[11px] text-foreground/40 mr-1">
              {t('resumes.generated.export')}
            </span>

            {/* Format picker */}
            <SegmentedControl<ExportFormat>
              ariaLabel={t('resumes.generated.format')}
              size="sm"
              value={exportFormat}
              onChange={setExportFormat}
              options={EXPORT_FORMATS.map((fmt) => ({ value: fmt, label: fmt.toUpperCase() }))}
            />

            {/* Template picker — only for pdf/docx */}
            {exportFormat !== 'txt' && (
              <SegmentedControl<TemplateId>
                ariaLabel={t('resumes.generated.template')}
                size="sm"
                value={exportTemplate}
                onChange={setExportTemplate}
                options={TEMPLATE_OPTIONS.map(({ id, label }) => ({ value: id, label }))}
              />
            )}

            <div className="flex items-center gap-1 ml-auto">
              {resumeDraft && (
                <Button
                  disabled={exporting === 'resume'}
                  onClick={() => void doExport('resume')}
                  className="flex items-center gap-1 rounded-lg bg-brand/10 border-brand/20 px-2.5 py-1.5 text-[11px] text-brand-soft transition-colors hover:bg-brand/20 h-auto"
                >
                  {exporting === 'resume' ? (
                    <Loader2 size={10} className="animate-spin" />
                  ) : (
                    <Download size={10} />
                  )}
                  {t('resumes.generated.exportResume')}
                </Button>
              )}
              {coverDraft && (
                <Button
                  disabled={exporting === 'cover'}
                  onClick={() => void doExport('cover')}
                  className="flex items-center gap-1 rounded-lg bg-white/5 border-white/[0.06] px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto"
                >
                  {exporting === 'cover' ? (
                    <Loader2 size={10} className="animate-spin" />
                  ) : (
                    <Download size={10} />
                  )}
                  {t('resumes.generated.exportCoverLetter')}
                </Button>
              )}
            </div>
          </div>
        )}

        {/* Top requirements */}
        {gen.topRequirements.length > 0 && (
          <div className="px-4 pb-3 flex flex-wrap gap-1">
            {gen.topRequirements.slice(0, 8).map((req) => (
              <span
                key={req}
                className="rounded-full border border-white/[0.06] bg-white/[0.03] px-2 py-0.5 text-[10px] text-foreground/50"
              >
                {req}
              </span>
            ))}
          </div>
        )}

        {/* Expandable sections. Resume + cover letter are editable (F1 + inline
            rewrite); the job ad and company brief stay read-only references. */}
        {(
          [
            {
              key: 'resume' as const,
              label: t('resumes.generated.resume'),
              text: resumeDraft,
              icon: FileText,
              editType: 'resume' as const,
              docType: 'resume' as const,
            },
            {
              key: 'cover' as const,
              label: t('resumes.generated.coverLetter'),
              text: coverDraft,
              icon: FileText,
              editType: 'cover' as const,
              docType: 'cover-letter' as const,
            },
            {
              key: 'jobAd' as const,
              label: t('resumes.generated.jobAd'),
              text: gen.jobAd,
              icon: Building2,
              editType: null,
              docType: null,
            },
            {
              key: 'brief' as const,
              label: t('resumes.generated.companyResearch'),
              text: gen.companyBrief,
              icon: Search,
              editType: null,
              docType: null,
            },
          ] as const
        )
          .filter((s) => s.text)
          .map(({ key, label, text, icon: SectionIcon, editType, docType }) => (
            <div key={key} className="border-t border-white/[0.04]">
              <Button
                variant="unstyled"
                onClick={() => setExpanded(expanded === key ? null : key)}
                className="flex w-full items-center justify-between px-4 py-2.5 text-left text-xs text-foreground/50 hover:text-foreground/70 transition-colors"
              >
                <span className="flex items-center gap-1.5">
                  <SectionIcon size={11} /> {label}
                </span>
                <ChevronDown
                  size={12}
                  className={cn('transition-transform', expanded === key && 'rotate-180')}
                />
              </Button>
              <AnimatePresence initial={false}>
                {expanded === key && (
                  <motion.div
                    initial={{ height: 0 }}
                    animate={{ height: 'auto' }}
                    exit={{ height: 0 }}
                    transition={transition.normal}
                    className="overflow-hidden"
                  >
                    {editType && docType ? (
                      <div className="flex h-72 flex-col px-4 pb-4">
                        <EditableOutput
                          value={text}
                          onChange={(v) => onEdit(editType, v)}
                          docType={docType}
                          meta={meta}
                          className="flex h-full flex-col overflow-hidden"
                          textAreaClassName="h-full w-full bg-transparent font-mono text-[10px] leading-relaxed text-foreground/60 placeholder:text-foreground/20"
                        />
                      </div>
                    ) : (
                      <pre className="select-text px-4 pb-4 whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-foreground/50 max-h-64 overflow-y-auto">
                        {text}
                      </pre>
                    )}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          ))}

        {/* Application answers — structured Q/A from the questions assistant. */}
        {gen.applicationAnswers.length > 0 && (
          <div className="border-t border-white/[0.04]">
            <Button
              variant="unstyled"
              onClick={() => setExpanded(expanded === 'answers' ? null : 'answers')}
              className="flex w-full items-center justify-between px-4 py-2.5 text-left text-xs text-foreground/50 transition-colors hover:text-foreground/70"
            >
              <span className="flex items-center gap-1.5">
                <HelpCircle size={11} /> {t('resumes.generated.applicationAnswers')}
                <span className="rounded-full bg-white/[0.06] px-1.5 py-0.5 text-[9px] text-foreground/45">
                  {gen.applicationAnswers.length}
                </span>
              </span>
              <ChevronDown
                size={12}
                className={cn('transition-transform', expanded === 'answers' && 'rotate-180')}
              />
            </Button>
            <AnimatePresence initial={false}>
              {expanded === 'answers' && (
                <motion.div
                  initial={{ height: 0 }}
                  animate={{ height: 'auto' }}
                  exit={{ height: 0 }}
                  transition={transition.normal}
                  className="overflow-hidden"
                >
                  <div className="select-text max-h-72 space-y-3 overflow-y-auto px-4 pb-4">
                    {gen.applicationAnswers.map((qa) => (
                      <div key={qa.id}>
                        <p className="text-[11px] font-medium text-foreground/70">{qa.question}</p>
                        <p className="mt-0.5 whitespace-pre-wrap text-[11px] leading-relaxed text-foreground/50">
                          {qa.answer}
                        </p>
                      </div>
                    ))}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        )}
      </GlassCard>

      <ConfirmModal
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={handleDelete}
        title={t('resumes.generated.deleteTitle')}
        description={t('resumes.generated.deleteDescription')}
        confirmText={t('resumes.generated.delete')}
        variant="danger"
        isConfirming={removeAiGeneration.isPending}
      />
    </>
  );
}
