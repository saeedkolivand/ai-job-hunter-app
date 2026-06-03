import { Check, Copy, Download, Eye, FileText, Pencil, RotateCcw } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useState } from 'react';

import { Button, cn, MarkdownMessage, TextArea } from '@ajh/ui';

import { buildFilename, type GenerationMeta, MODES, type TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { ExportModal } from '../ExportModal';

interface OutputPanelDoneProps {
  resumeOut: string;
  coverOut: string;
  activeOut: 'resume' | 'cover';
  meta: GenerationMeta | null;
  mode: string;
  templateId: TemplateId;
  onActiveOutChange: (out: 'resume' | 'cover') => void;
  onCopy: () => void;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => Promise<void>;
  onOutputChange: (value: string) => void;
  onRegenerate: () => void;
  copied: boolean;
  isGenerating?: boolean;
}

export function OutputPanelDone({
  resumeOut,
  coverOut,
  activeOut,
  meta,
  mode,
  templateId: _templateId,
  onActiveOutChange,
  onCopy,
  onExport,
  onOutputChange,
  onRegenerate,
  copied,
  isGenerating = false,
}: OutputPanelDoneProps) {
  const { t } = useTranslation();
  const [exportOpen, setExportOpen] = useState(false);
  // Preview (prettified markdown) vs Edit (raw text). Display-only: the raw
  // string stays canonical — copy, export, and the **keyword** emphasis markers
  // are never touched, so a Preview/Edit switch can't change the exported file.
  const [view, setView] = useState<'preview' | 'edit'>('preview');

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
              ...(resumeOut ? [{ id: 'resume' as const, label: t('aiGenerate.resume') }] : []),
              ...(coverOut ? [{ id: 'cover' as const, label: t('aiGenerate.coverLetter') }] : []),
            ] as { id: 'resume' | 'cover'; label: string }[]
          ).map(({ id, label }) => (
            <Button
              key={id}
              onClick={() => onActiveOutChange(id)}
              className={cn(
                'rounded-lg px-3 py-1.5 text-xs font-medium transition-all h-auto',
                activeOut === id
                  ? 'bg-brand/15 text-brand-soft'
                  : 'text-foreground/45 hover:text-foreground/70'
              )}
            >
              {label}
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

      {/* Filename preview */}
      {currentMeta && (
        <div className="shrink-0 border-b border-white/[0.05] px-6 py-2 flex items-center gap-2 text-[10px] text-foreground/30">
          <FileText size={10} />
          <span className="font-mono">
            {buildFilename(currentMeta, activeOut === 'resume' ? 'resume' : 'cover-letter', 'pdf')}
          </span>
        </div>
      )}

      {/* Output — prettified Preview or raw Edit (display-only; export uses the raw text) */}
      <div className="flex flex-1 flex-col overflow-hidden px-6 py-4">
        <div className="mb-2 flex shrink-0 items-center gap-0.5 self-end rounded-lg bg-white/[0.04] p-0.5">
          <Button
            variant="unstyled"
            onClick={() => setView('preview')}
            className={cn(
              'flex items-center gap-1 rounded px-2 py-1 text-[10px] transition-colors',
              view === 'preview'
                ? 'bg-brand/15 text-brand-soft'
                : 'text-foreground/45 hover:text-foreground/70'
            )}
          >
            <Eye size={11} /> {t('aiGenerate.preview')}
          </Button>
          <Button
            variant="unstyled"
            onClick={() => setView('edit')}
            className={cn(
              'flex items-center gap-1 rounded px-2 py-1 text-[10px] transition-colors',
              view === 'edit'
                ? 'bg-brand/15 text-brand-soft'
                : 'text-foreground/45 hover:text-foreground/70'
            )}
          >
            <Pencil size={11} /> {t('aiGenerate.edit')}
          </Button>
        </div>

        {view === 'edit' ? (
          <TextArea
            value={currentOutput}
            onChange={(e) => onOutputChange(e.target.value)}
            className="h-full w-full bg-transparent font-mono text-[12px] leading-relaxed text-foreground/80 placeholder:text-foreground/20"
            spellCheck={false}
            placeholder={t('aiGenerate.placeholder')}
          />
        ) : (
          <div className="h-full w-full overflow-y-auto rounded-lg">
            {currentOutput ? (
              <MarkdownMessage content={currentOutput} className="text-[12px] text-foreground/80" />
            ) : (
              <p className="text-[12px] text-foreground/20">{t('aiGenerate.placeholder')}</p>
            )}
          </div>
        )}
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
