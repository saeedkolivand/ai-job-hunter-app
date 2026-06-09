import { Check, Copy, Download, FileText, LayoutTemplate, Loader2, RotateCcw } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useState } from 'react';

import { Button, cn } from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import {
  buildFilename,
  type GenerationMeta,
  MODES,
  resolveMarket,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { ExportModal } from '../ExportModal';
import { PdfPreview } from '../PdfPreview';

interface OutputPanelDoneProps {
  resumeOut: string;
  coverOut: string;
  activeOut: 'resume' | 'cover';
  meta: GenerationMeta | null;
  mode: string;
  templateId: TemplateId;
  /** ATS single-column override — must match the export so the preview is faithful. */
  atsMode: boolean;
  /** Export market/locale — drives the cover-letter preview layout (#24). */
  locale?: string;
  onActiveOutChange: (out: 'resume' | 'cover') => void;
  onCopy: () => void;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => Promise<void>;
  onOutputChange: (value: string) => void;
  onRegenerate: () => void;
  copied: boolean;
  isGenerating?: boolean;
  /** Which document is still streaming during progressive reveal (#23), or null. */
  generatingDoc?: 'resume' | 'cover' | null;
}

export function OutputPanelDone({
  resumeOut,
  coverOut,
  activeOut,
  meta,
  mode,
  templateId,
  atsMode,
  locale,
  onActiveOutChange,
  onCopy,
  onExport,
  onOutputChange,
  onRegenerate,
  copied,
  isGenerating = false,
  generatingDoc = null,
}: OutputPanelDoneProps) {
  const { t } = useTranslation();
  const [exportOpen, setExportOpen] = useState(false);

  const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';
  // The cover letter's preview layout follows the resolved market (job country →
  // language → override), exactly like the real export; résumé keeps the locale.
  const previewLocale =
    docType === 'cover-letter'
      ? resolveMarket({
          jobCountry: meta?.jobCountry,
          targetLanguage: meta?.targetLanguage,
          override: locale,
        })
      : locale;

  // If the active tab has no content but the other does, switch automatically
  useEffect(() => {
    if (activeOut === 'resume' && !resumeOut && coverOut) onActiveOutChange('cover');
    if (activeOut === 'cover' && !coverOut && resumeOut) onActiveOutChange('resume');
  }, [activeOut, resumeOut, coverOut, onActiveOutChange]);

  const currentOutput = activeOut === 'resume' ? resumeOut : coverOut;
  const currentMeta = meta;

  return (
    <motion.div
      key="done"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Output toolbar */}
      <div className="shrink-0 flex items-center justify-between border-b border-white/[0.05] px-6 py-3">
        <div className="flex items-center gap-1">
          {(
            [
              ...(resumeOut || generatingDoc === 'resume'
                ? [{ id: 'resume' as const, label: t('aiGenerate.resume') }]
                : []),
              ...(coverOut || generatingDoc === 'cover'
                ? [{ id: 'cover' as const, label: t('aiGenerate.coverLetter') }]
                : []),
            ] as { id: 'resume' | 'cover'; label: string }[]
          ).map(({ id, label }) => (
            <Button
              key={id}
              onClick={() => onActiveOutChange(id)}
              className={cn(
                'inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all h-auto',
                activeOut === id
                  ? 'bg-brand/15 text-brand-soft'
                  : 'text-foreground/45 hover:text-foreground/70'
              )}
            >
              {label}
              {/* #23 — the document still streaming in the background gets a spinner. */}
              {generatingDoc === id && (
                <Loader2 size={10} className="animate-spin text-brand-soft" />
              )}
            </Button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <Button
            onClick={onCopy}
            disabled={isGenerating}
            className="flex items-center gap-1.5 rounded-lg bg-white/[0.04] px-2.5 py-1.5 text-[11px] text-foreground/55 hover:text-foreground transition-colors h-auto disabled:opacity-30 disabled:pointer-events-none"
          >
            {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />}
            {copied ? t('aiGenerate.copied') : t('aiGenerate.copy')}
          </Button>
          <Button
            onClick={() => setExportOpen(true)}
            disabled={isGenerating}
            className="flex items-center gap-1.5 rounded-lg bg-brand/15 px-2.5 py-1.5 text-[11px] font-medium text-brand-soft hover:bg-brand/20 transition-colors h-auto disabled:opacity-30 disabled:pointer-events-none"
          >
            <Download size={11} />
            {t('aiGenerate.export')}
          </Button>
        </div>
      </div>

      {/* Filename preview + active template (#12 — template info on top of the
          resume box; templates apply to résumés, so it's hidden for cover letters). */}
      {(currentMeta || activeOut === 'resume') && (
        <div className="shrink-0 border-b border-white/[0.05] px-6 py-2 flex items-center gap-2 text-[10px] text-foreground/30">
          {currentMeta && (
            <>
              <FileText size={10} />
              <span className="font-mono">
                {buildFilename(
                  currentMeta,
                  activeOut === 'resume' ? 'resume' : 'cover-letter',
                  'pdf'
                )}
              </span>
            </>
          )}
          {activeOut === 'resume' && (
            <span className="ml-auto flex items-center gap-1 rounded bg-white/[0.04] px-1.5 py-0.5 text-foreground/45">
              <LayoutTemplate size={10} />
              {t('aiGenerate.templateLabel')}: {TEMPLATES[templateId].name}
            </span>
          )}
        </div>
      )}

      {/* Editing locked while cover letter is still streaming (#23 progressive reveal). */}
      {generatingDoc === 'cover' && (
        <div className="shrink-0 flex items-center gap-2 border-b border-amber-400/20 bg-amber-400/5 px-6 py-2 text-[11px] text-amber-400/80">
          <Loader2 size={11} className="animate-spin shrink-0" />
          Cover letter is still generating — editing locked until both documents are ready.
        </div>
      )}

      {/* Output — prettified Preview or raw Edit; the raw string stays canonical,
          so copy/export read exactly what's edited (incl. inline AI rewrites). */}
      <div className="flex flex-1 flex-col overflow-hidden px-6 py-4">
        <EditableOutput
          value={currentOutput}
          onChange={onOutputChange}
          disabled={isGenerating}
          docType={docType}
          meta={meta}
          className="flex h-full w-full flex-col overflow-hidden"
          previewSlot={
            <PdfPreview
              text={currentOutput}
              docType={docType}
              meta={meta}
              templateId={templateId}
              atsMode={atsMode}
              locale={previewLocale}
              paused={generatingDoc === activeOut}
              className="h-full w-full"
            />
          }
        />
      </div>

      {/* Re-generate option */}
      <div className="shrink-0 border-t border-white/[0.05] px-6 py-3 flex items-center justify-between">
        <span className="text-[10px] text-foreground/30">
          {MODES[mode as keyof typeof MODES].label} · {meta?.targetLanguage?.toUpperCase() ?? 'EN'}
          {meta?.mismatch && ` · ${t('aiGenerate.localized')}`}
        </span>
        <Button
          onClick={onRegenerate}
          className="flex items-center gap-1.5 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
        >
          <RotateCcw size={11} /> {t('aiGenerate.regenerate')}
        </Button>
      </div>

      <ExportModal
        open={exportOpen}
        onClose={() => setExportOpen(false)}
        meta={meta}
        docType={activeOut === 'resume' ? 'resume' : 'cover-letter'}
        onExport={onExport}
      />
    </motion.div>
  );
}
