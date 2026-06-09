import { FileWarning, Loader2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '@ajh/ui';

import { type GenerationMeta, renderPdfPreview, type TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

interface PdfPreviewProps {
  /** Canonical document text (the same raw string copy/export read). */
  text: string;
  docType: 'resume' | 'cover-letter';
  meta?: GenerationMeta | null;
  templateId: TemplateId;
  atsMode?: boolean;
  /** Export market/locale (resolved by the caller, mirroring the real export). */
  locale?: string;
  /** Skip rendering — e.g. while the document is still generating. */
  paused?: boolean;
  className?: string;
}

/** How long after the last edit to re-render the real PDF (#24). */
const DEBOUNCE_MS = 500;

type Status = 'idle' | 'rendering' | 'ready' | 'error';

/**
 * Live preview that renders the **real exported PDF** (not an approximation):
 * it calls the same Rust renderer the export uses (`renderPdfPreview`), wraps the
 * returned bytes in a `blob:` URL, and shows them in an `<iframe>` (the webview's
 * native PDF viewer). Re-renders ~{@link DEBOUNCE_MS}ms after edits settle, so
 * typing stays instant while the preview catches up. The last good PDF stays on
 * screen during a re-render (overlay spinner) and after a transient failure.
 *
 * This is the authoritative output before download — see ADR-012. The `blob:`
 * frame source requires `frame-src blob:` in the CSP (tauri.conf.json).
 */
export function PdfPreview({
  text,
  docType,
  meta,
  templateId,
  atsMode = false,
  locale,
  paused = false,
  className,
}: PdfPreviewProps) {
  const { t } = useTranslation();
  const [url, setUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<Status>('idle');
  // Monotonic token so a slow render that resolves after a newer edit is ignored.
  const renderToken = useRef(0);
  const urlRef = useRef<string | null>(null);

  // Revoke the last object URL on unmount.
  useEffect(
    () => () => {
      if (urlRef.current) URL.revokeObjectURL(urlRef.current);
    },
    []
  );

  useEffect(() => {
    if (paused || !text.trim()) {
      setStatus('idle');
      return;
    }
    const token = ++renderToken.current;
    setStatus('rendering');
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const bytes = await renderPdfPreview(
            text,
            docType,
            meta ?? undefined,
            templateId,
            atsMode,
            locale
          );
          if (token !== renderToken.current) return; // superseded by a newer edit
          // Swap the blob only when the new one is ready, revoking the previous.
          const next = URL.createObjectURL(new Blob([bytes], { type: 'application/pdf' }));
          if (urlRef.current) URL.revokeObjectURL(urlRef.current);
          urlRef.current = next;
          setUrl(next);
          setStatus('ready');
        } catch {
          if (token !== renderToken.current) return;
          setStatus('error');
        }
      })();
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [text, docType, meta, templateId, atsMode, locale, paused]);

  return (
    <div
      className={cn(
        'relative h-full w-full overflow-hidden rounded-lg border border-white/[0.06] bg-white/[0.02]',
        className
      )}
    >
      {url ? (
        <iframe
          src={url}
          title={t('aiGenerate.pdfPreview.title')}
          className="h-full w-full border-0 bg-white"
        />
      ) : status === 'error' ? (
        <div className="flex h-full flex-col items-center justify-center gap-2 text-foreground/40">
          <FileWarning size={20} />
          <p className="text-[12px]">{t('aiGenerate.pdfPreview.failed')}</p>
        </div>
      ) : status === 'rendering' ? (
        <div className="flex h-full flex-col items-center justify-center gap-2 text-foreground/40">
          <Loader2 size={18} className="animate-spin text-brand-soft" />
          <p className="text-[12px]">{t('aiGenerate.pdfPreview.rendering')}</p>
        </div>
      ) : (
        <div className="flex h-full items-center justify-center text-[12px] text-foreground/25">
          {t('aiGenerate.pdfPreview.empty')}
        </div>
      )}

      {/* Keep the last good PDF visible during a re-render / after a transient
          failure, with a corner indicator instead of blanking the pane. */}
      {url && status === 'rendering' && (
        <div className="absolute right-2 top-2 flex items-center gap-1.5 rounded-lg bg-black/55 px-2 py-1 text-[10px] text-white/85 backdrop-blur-sm">
          <Loader2 size={10} className="animate-spin" />
          {t('aiGenerate.pdfPreview.rendering')}
        </div>
      )}
      {url && status === 'error' && (
        <div className="absolute right-2 top-2 flex items-center gap-1.5 rounded-lg bg-red-500/70 px-2 py-1 text-[10px] text-white/90 backdrop-blur-sm">
          <FileWarning size={10} />
          {t('aiGenerate.pdfPreview.failed')}
        </div>
      )}
    </div>
  );
}
