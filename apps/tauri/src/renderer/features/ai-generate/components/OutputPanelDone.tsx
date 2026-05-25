import { Check, Copy, Download, FileText, Loader2, RotateCcw } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { Button, TextArea } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { buildFilename, type GenerationMeta, MODES, type TemplateId } from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';

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
}: OutputPanelDoneProps) {
  const { t } = useTranslation();

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
            className="flex items-center gap-1.5 rounded-lg bg-white/[0.04] px-2.5 py-1.5 text-[11px] text-foreground/55 hover:text-foreground transition-colors h-auto"
          >
            {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />}
            {copied ? t('aiGenerate.copied') : t('aiGenerate.copy')}
          </Button>
          <ExportMenu onExport={onExport} t={t} />
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

      {/* Editable output */}
      <div className="flex-1 overflow-hidden px-6 py-4">
        <TextArea
          value={currentOutput}
          onChange={(e) => onOutputChange(e.target.value)}
          className="h-full w-full bg-transparent font-mono text-[12px] leading-relaxed text-foreground/80 placeholder:text-foreground/20"
          spellCheck={false}
          placeholder={t('aiGenerate.placeholder')}
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
    </motion.div>
  );
}

// ─── Export menu ──────────────────────────────────────────────────────────────

function ExportMenu({
  onExport,
  t,
}: {
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => Promise<void>;
  t: (key: string, params?: Record<string, unknown>) => string;
}) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState<string | null>(null);

  const handle = async (fmt: 'pdf' | 'docx' | 'txt') => {
    setLoading(fmt);
    setOpen(false);
    try {
      await onExport(fmt);
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className="relative">
      <Button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1.5 rounded-lg bg-brand/15 px-2.5 py-1.5 text-[11px] font-medium text-brand-soft hover:bg-brand/20 transition-colors h-auto"
      >
        {loading ? <Loader2 size={11} className="animate-spin" /> : <Download size={11} />}
        {t('aiGenerate.export')}
      </Button>
      <AnimatePresence>
        {open && (
          <>
            <div className="fixed inset-0 z-[var(--z-dropdown)]" onClick={() => setOpen(false)} />
            <motion.div
              initial={{ opacity: 0, y: -4, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.97 }}
              transition={transition.fast}
              className="absolute right-0 top-full z-[var(--z-modal)] mt-1.5 w-36 overflow-hidden rounded-xl border border-white/10 bg-secondary shadow-2xl"
            >
              {(['pdf', 'docx', 'txt'] as const).map((fmt) => (
                <Button
                  key={fmt}
                  onClick={() => void handle(fmt)}
                  className="flex w-full items-center gap-2.5 px-3 py-2.5 text-xs text-foreground/65 hover:bg-white/[0.05] hover:text-foreground transition-colors h-auto rounded-none border-none bg-transparent"
                >
                  <Download size={11} />
                  {t('aiGenerate.download', { fmt: fmt.toUpperCase() })}
                </Button>
              ))}
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}
