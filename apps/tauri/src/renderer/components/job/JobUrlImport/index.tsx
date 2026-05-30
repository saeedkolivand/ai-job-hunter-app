import { AlertCircle, Link2, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { Button, Input } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useImportJobUrl } from '@/services';

interface Props {
  /** Receives the composed job-ad text when a URL resolves to a description. */
  onImport: (text: string) => void;
  disabled?: boolean;
}

/**
 * Paste a job posting URL (LinkedIn, Greenhouse, Lever, …) and pull its full
 * description into the job-ad field, reusing the backend `scrape.resolveUrl`
 * resolver. Used by AI Generate and the Resume Analyzer.
 */
export function JobUrlImport({ onImport, disabled }: Props) {
  const { t } = useTranslation();
  const importJob = useImportJobUrl();
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);

  const busy = importJob.isPending;

  const handleImport = async () => {
    const trimmed = url.trim();
    if (!trimmed || disabled || busy) return;
    setError(null);
    try {
      const posting = await importJob.mutateAsync(trimmed);
      const description = posting?.description?.trim();
      if (!description) {
        setError(t('jobUrlImport.notFound'));
        return;
      }
      const header = [posting?.title, posting?.company].filter(Boolean).join(' — ');
      onImport(header ? `${header}\n\n${description}` : description);
      setUrl('');
    } catch {
      setError(t('jobUrlImport.failed'));
    }
  };

  return (
    <div className="space-y-1.5">
      <div className="flex gap-1.5">
        <Input
          autoFocus
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void handleImport();
            }
          }}
          placeholder={t('jobUrlImport.placeholder')}
          disabled={disabled || busy}
          className="flex-1 rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none transition-colors focus:border-brand/40"
        />
        <Button
          onClick={() => void handleImport()}
          disabled={disabled || busy || !url.trim()}
          className="flex h-auto shrink-0 items-center gap-1.5 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2 text-xs font-medium text-foreground/70 transition-colors hover:border-white/10 hover:text-foreground/90 disabled:opacity-40"
        >
          {busy ? <Loader2 size={12} className="animate-spin" /> : <Link2 size={12} />}
          {t('jobUrlImport.import')}
        </Button>
      </div>
      {error && (
        <div className="flex items-center gap-1.5 text-[10px] text-amber-300/80">
          <AlertCircle size={10} className="shrink-0" />
          {error}
        </div>
      )}
    </div>
  );
}
