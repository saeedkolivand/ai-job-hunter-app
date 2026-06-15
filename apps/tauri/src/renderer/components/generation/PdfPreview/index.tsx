import { FileWarning, Loader2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { cn, Image } from '@ajh/ui';

import { type GenerationMeta, renderDocumentPreview, type TemplateId } from '@/lib/generate';

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

/**
 * Settle delay before re-rendering the preview. The incoming `text` now changes
 * only on discrete events (initial generation, regeneration, explicit Save,
 * doc/template/locale switch) — never per keystroke — so no throttling is needed;
 * a fresh commit recompiles immediately.
 */
const DEBOUNCE_MS = 0;

type Status = 'idle' | 'rendering' | 'ready' | 'error';

/**
 * Live preview that renders the **real exported document** (not an approximation):
 * it calls the same Rust Typst renderer the export uses (`renderDocumentPreview`),
 * and shows the returned SVG pages as stacked `<img>` elements (one per page) in a
 * scrollable container. The `text` it receives is the COMMITTED document text,
 * which the caller refreshes only on discrete events (initial generation,
 * regeneration, explicit Save, doc/template/locale switch) — not on every
 * keystroke — so each change triggers a single recompile. The last good pages stay
 * on screen during a re-render (corner spinner overlay) and after a transient failure.
 *
 * This is the authoritative output before download — see ADR-012.
 * SVGs are rendered via Blob object URLs (`URL.createObjectURL`) — revoked on each
 * new render batch and on unmount; CSP `img-src 'self' blob:` allows this.
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
  const [pageUrls, setPageUrls] = useState<string[]>([]);
  const [status, setStatus] = useState<Status>('idle');
  // Monotonic token so a slow render that resolves after a newer edit is ignored.
  const renderToken = useRef(0);
  // Track current object URLs so we can revoke them when replaced or on unmount.
  const urlsRef = useRef<string[]>([]);
  // Track last rendered docType to detect doc-switch vs. same-doc edit.
  const prevDocTypeRef = useRef<string | null>(null);

  // Revoke all object URLs on unmount.
  useEffect(() => {
    return () => {
      urlsRef.current.forEach((url) => URL.revokeObjectURL(url));
    };
  }, []);

  useEffect(() => {
    if (paused || !text.trim()) {
      setStatus('idle');
      return;
    }

    // Detect a doc switch (résumé ↔ cover letter). On first render prevDocTypeRef
    // is null — treat that as a non-switch so we don't clear before the initial load.
    const isDocSwitch = prevDocTypeRef.current !== null && prevDocTypeRef.current !== docType;
    prevDocTypeRef.current = docType;

    if (isDocSwitch) {
      // Clear stale pages immediately so the centered loader shows instead of the
      // previous doc's image. Revoke old blob URLs to avoid leaking memory.
      urlsRef.current.forEach((url) => URL.revokeObjectURL(url));
      urlsRef.current = [];
      setPageUrls([]);
    }

    const token = ++renderToken.current;
    setStatus('rendering');
    // Doc switch: render immediately (nothing to debounce). Same-doc edit: debounce.
    const delay = isDocSwitch ? 0 : DEBOUNCE_MS;
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const result = await renderDocumentPreview(
            text,
            docType,
            meta ?? undefined,
            templateId,
            atsMode,
            locale
          );
          if (token !== renderToken.current) return; // superseded by a newer edit
          // Revoke old batch before creating new URLs.
          urlsRef.current.forEach((url) => URL.revokeObjectURL(url));
          const newUrls = result.map((svg) =>
            URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }))
          );
          urlsRef.current = newUrls;
          setPageUrls(newUrls);
          setStatus('ready');
        } catch {
          if (token !== renderToken.current) return;
          setStatus('error');
        }
      })();
    }, delay);
    return () => clearTimeout(timer);
  }, [text, docType, meta, templateId, atsMode, locale, paused]);

  const hasPages = pageUrls.length > 0;
  const title = t('aiGenerate.pdfPreview.title');

  return (
    <div
      className={cn(
        'relative h-full w-full overflow-hidden rounded-lg border border-white/[0.06] bg-white/[0.02]',
        className
      )}
    >
      {hasPages ? (
        <div className="h-full w-full overflow-y-auto">
          <Image.PreviewGroup>
            <div className="flex flex-col items-center gap-4 p-4">
              {pageUrls.map((url, i) => (
                <Image
                  key={i}
                  src={url}
                  alt={`${title} — page ${i + 1}`}
                  rootClassName="w-full rounded bg-white shadow-sm"
                  className="w-full"
                />
              ))}
            </div>
          </Image.PreviewGroup>
        </div>
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

      {/* Keep last good pages visible during re-render / after transient failure,
          with a corner indicator instead of blanking the pane. */}
      {hasPages && status === 'rendering' && (
        <div className="absolute right-2 top-2 flex items-center gap-1.5 rounded-lg bg-black/55 px-2 py-1 text-[10px] text-white/85 backdrop-blur-sm">
          <Loader2 size={10} className="animate-spin" />
          {t('aiGenerate.pdfPreview.rendering')}
        </div>
      )}
      {hasPages && status === 'error' && (
        <div className="absolute right-2 top-2 flex items-center gap-1.5 rounded-lg bg-red-500/70 px-2 py-1 text-[10px] text-white/90 backdrop-blur-sm">
          <FileWarning size={10} />
          {t('aiGenerate.pdfPreview.failed')}
        </div>
      )}
    </div>
  );
}
