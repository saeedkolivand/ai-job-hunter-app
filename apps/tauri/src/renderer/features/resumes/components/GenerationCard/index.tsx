import {
  Building2,
  Calendar,
  ChevronDown,
  Copy,
  Download,
  FileText,
  Loader2,
  Trash2,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { Button } from '@ajh/ui';

import { cn } from '@/lib/cn';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type TemplateId,
} from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useRemoveAiGeneration } from '@/services/use-ai-generations';

const EXPORT_FORMATS = ['pdf', 'docx', 'txt'] as const;
type ExportFormat = (typeof EXPORT_FORMATS)[number];

const TEMPLATE_OPTIONS: { id: TemplateId; label: string }[] = [
  { id: 'modern', label: 'Modern' },
  { id: 'classic', label: 'Classic' },
  { id: 'executive', label: 'Executive' },
];

interface GenerationCardProps {
  gen: AiGenerationRecord;
}

export function GenerationCard({ gen }: GenerationCardProps) {
  const { t } = useTranslation();
  const removeAiGeneration = useRemoveAiGeneration();
  const [expanded, setExpanded] = useState<'resume' | 'cover' | 'jobAd' | null>(null);
  const [copied, setCopied] = useState<'resume' | 'cover' | null>(null);
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');
  const [exportTemplate, setExportTemplate] = useState<TemplateId>('modern');
  const [exporting, setExporting] = useState<'resume' | 'cover' | null>(null);

  const copy = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? gen.resumeText : gen.coverLetterText;
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
    const text = type === 'resume' ? gen.resumeText : gen.coverLetterText;
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
    <div className="glass-graphite glass-highlight rounded-xl overflow-hidden">
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
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {gen.resumeText && (
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
          {gen.coverLetterText && (
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
            onClick={() => void removeAiGeneration.mutate(gen.id)}
            className="flex items-center gap-1 rounded-lg bg-white/5 px-2 py-1.5 text-[11px] text-foreground/40 transition-colors hover:text-red-400 h-auto border-transparent"
          >
            <Trash2 size={10} />
          </Button>
        </div>
      </div>

      {/* Export bar */}
      {(gen.resumeText || gen.coverLetterText) && (
        <div className="border-t border-white/[0.04] px-4 py-2.5 flex items-center gap-2 flex-wrap">
          <Download size={11} className="text-foreground/30 shrink-0" />
          <span className="text-[11px] text-foreground/40 mr-1">
            {t('resumes.generated.export')}
          </span>

          {/* Format picker */}
          <div className="flex items-center gap-0.5 rounded-lg border border-white/[0.06] bg-white/[0.03] p-0.5">
            {EXPORT_FORMATS.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setExportFormat(fmt)}
                className={cn(
                  'rounded-md px-2 py-0.5 text-[10px] uppercase tracking-wider transition-colors',
                  exportFormat === fmt
                    ? 'bg-white/10 text-foreground/80'
                    : 'text-foreground/35 hover:text-foreground/60'
                )}
              >
                {fmt}
              </button>
            ))}
          </div>

          {/* Template picker — only for pdf/docx */}
          {exportFormat !== 'txt' && (
            <div className="flex items-center gap-0.5 rounded-lg border border-white/[0.06] bg-white/[0.03] p-0.5">
              {TEMPLATE_OPTIONS.map(({ id, label }) => (
                <button
                  key={id}
                  onClick={() => setExportTemplate(id)}
                  className={cn(
                    'rounded-md px-2 py-0.5 text-[10px] transition-colors',
                    exportTemplate === id
                      ? 'bg-white/10 text-foreground/80'
                      : 'text-foreground/35 hover:text-foreground/60'
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          )}

          <div className="flex items-center gap-1 ml-auto">
            {gen.resumeText && (
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
            {gen.coverLetterText && (
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

      {/* Expandable sections */}
      {[
        {
          key: 'resume' as const,
          label: t('resumes.generated.resume'),
          text: gen.resumeText,
          icon: FileText,
        },
        {
          key: 'cover' as const,
          label: t('resumes.generated.coverLetter'),
          text: gen.coverLetterText,
          icon: FileText,
        },
        {
          key: 'jobAd' as const,
          label: t('resumes.generated.jobAd'),
          text: gen.jobAd,
          icon: Building2,
        },
      ]
        .filter((s) => s.text)
        .map(({ key, label, text, icon: SectionIcon }) => (
          <div key={key} className="border-t border-white/[0.04]">
            <button
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
            </button>
            <AnimatePresence initial={false}>
              {expanded === key && (
                <motion.div
                  initial={{ height: 0 }}
                  animate={{ height: 'auto' }}
                  exit={{ height: 0 }}
                  transition={transition.normal}
                  className="overflow-hidden"
                >
                  <pre className="px-4 pb-4 whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-foreground/50 max-h-64 overflow-y-auto">
                    {text}
                  </pre>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        ))}
    </div>
  );
}
