import { Download, FileText, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ModalShell } from '@ajh/ui';

import { buildFilename, type GenerationMeta } from '@/lib/generate';

type Fmt = 'pdf' | 'docx' | 'txt';

interface Props {
  open: boolean;
  onClose: () => void;
  meta: GenerationMeta | null;
  docType: 'resume' | 'cover-letter';
  onExport: (fmt: Fmt) => Promise<void>;
}

const FORMATS: { id: Fmt; descKey: string }[] = [
  { id: 'pdf', descKey: 'aiGenerate.exportPdfDesc' },
  { id: 'docx', descKey: 'aiGenerate.exportDocxDesc' },
  { id: 'txt', descKey: 'aiGenerate.exportTxtDesc' },
];

export function ExportModal({ open, onClose, meta, docType, onExport }: Props) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState<Fmt | null>(null);

  const handle = async (fmt: Fmt) => {
    if (loading) return;
    setLoading(fmt);
    try {
      await onExport(fmt);
      onClose();
    } finally {
      setLoading(null);
    }
  };

  return (
    <ModalShell
      open={open}
      onClose={onClose}
      maxWidth="max-w-sm"
      ariaLabel={t('aiGenerate.exportTitle')}
    >
      <div className="p-5">
        <div className="mb-1 flex items-center gap-2">
          <Download size={15} className="text-brand-soft" />
          <span className="text-sm font-semibold text-foreground/85">
            {t('aiGenerate.exportTitle')}
          </span>
        </div>
        <p className="mb-4 text-[11px] text-foreground/40">{t('aiGenerate.exportSubtitle')}</p>

        <div className="space-y-2">
          {FORMATS.map(({ id, descKey }) => {
            const filename = meta ? buildFilename(meta, docType, id) : `document.${id}`;
            return (
              <Button
                key={id}
                onClick={() => void handle(id)}
                disabled={loading !== null}
                className="flex w-full items-center gap-3 rounded-xl border border-[var(--border-clear)] bg-card px-4 py-3 text-left transition-colors hover:border-brand/30 hover:bg-brand/5 h-auto disabled:opacity-50"
              >
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-muted">
                  {loading === id ? (
                    <Loader2 size={14} className="animate-spin text-brand-soft" />
                  ) : (
                    <FileText size={14} className="text-foreground/50" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-xs font-semibold text-foreground/80">{id.toUpperCase()}</div>
                  <div className="text-[10px] text-foreground/40">{t(descKey)}</div>
                  <div className="mt-0.5 truncate font-mono text-[9px] text-foreground/30">
                    {filename}
                  </div>
                </div>
              </Button>
            );
          })}
        </div>
      </div>
    </ModalShell>
  );
}
