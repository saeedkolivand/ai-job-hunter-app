import { Loader2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

interface RewriteResultPreviewProps {
  streaming: boolean;
  result: string;
  error: string | null;
  stillWorking: boolean;
  unchanged: boolean;
}

/** Streaming preview / finished result / error plus the neutral status lines. */
export function RewriteResultPreview({
  streaming,
  result,
  error,
  stillWorking,
  unchanged,
}: RewriteResultPreviewProps) {
  const { t } = useTranslation();
  if (!streaming && !result && !error) return null;
  return (
    <div>
      <p className="mb-1 flex items-center gap-1 text-[9px] font-semibold uppercase tracking-wider text-foreground/35">
        {streaming && <Loader2 size={9} className="animate-spin" />}
        {streaming ? t('aiGenerate.rewrite.streaming') : t('aiGenerate.rewrite.resultLabel')}
      </p>
      {error ? (
        <p className="rounded-md bg-red-400/10 px-2 py-1.5 text-[11px] text-red-300">{error}</p>
      ) : (
        <p className="max-h-32 overflow-y-auto whitespace-pre-wrap rounded-md border border-brand/15 bg-brand/[0.04] px-2 py-1.5 text-[11px] leading-relaxed text-foreground/80">
          {result || '…'}
        </p>
      )}
      {/* A long reasoning pass looks dead without this line. */}
      {streaming && stillWorking && (
        <p role="status" aria-live="polite" className="mt-1 text-[10px] italic text-foreground/45">
          {t('aiGenerate.rewrite.stillWorking')}
        </p>
      )}
      {/* Neutral, NOT an error: the model handed the selection back. */}
      {unchanged && !streaming && (
        <p
          role="status"
          aria-live="polite"
          className="mt-1 rounded-md bg-muted px-2 py-1.5 text-[11px] text-foreground/60"
        >
          {t('aiGenerate.rewrite.unchanged')}
        </p>
      )}
    </div>
  );
}
