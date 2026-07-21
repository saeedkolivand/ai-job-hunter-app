import { AlertCircle, Check, Link2, Loader2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input } from '@ajh/ui';

import { useImportJobUrl } from '@/services';

/**
 * Provenance of a URL-imported job ad (ADR-031): the resolved posting's
 * canonical url + the board it came from. Surfaced up to the AI-generate state
 * so `persist()` can populate `AiGenerationSaveRequest.jobUrl`/`board`, joining
 * URL-imported generations into applied-detection + cluster provenance.
 */
export interface JobAdProvenance {
  /** The resolved posting's canonical url (falls back to the pasted url). */
  url: string;
  /** The board the posting came from (e.g. "greenhouse"); absent if unknown. */
  board?: string;
}

interface Props {
  /**
   * Receives the composed job-ad text when a URL resolves to a description,
   * plus the import provenance (url + board) so a caller can persist it.
   */
  onImport: (text: string, provenance: JobAdProvenance) => void;
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
  const inputRef = useRef<HTMLInputElement>(null);
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  // What the last successful import resolved to — shown so the user sees the
  // detected role and company, not just a silently-filled job-ad field.
  const [imported, setImported] = useState<{ title?: string; company?: string } | null>(null);

  const busy = importJob.isPending;

  // Focus-on-mount without the `autoFocus` prop (jsx-a11y/no-autofocus).
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

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
      // Persist the resolved canonical url (what found-jobs/harvest key on) so the
      // applied-detection join lands; fall back to the pasted url if absent.
      onImport(header ? `${header}\n\n${description}` : description, {
        url: posting?.url?.trim() || trimmed,
        board: posting?.source || undefined,
      });
      setImported({ title: posting?.title, company: posting?.company });
      setUrl('');
    } catch {
      setError(t('jobUrlImport.failed'));
    }
  };

  return (
    <div className="space-y-1.5">
      <div className="flex gap-1.5">
        <Input
          ref={inputRef}
          value={url}
          aria-invalid={!!error}
          onChange={(e) => {
            setUrl(e.target.value);
            if (error) setError(null);
            if (imported) setImported(null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void handleImport();
            }
          }}
          placeholder={t('jobUrlImport.placeholder')}
          disabled={disabled || busy}
          className="flex-1 rounded-lg border border-[var(--border-clear)] bg-field px-3 py-2 text-xs text-foreground/80 placeholder:text-foreground/25 outline-none transition-colors focus:border-brand/40"
        />
        <Button
          onClick={() => void handleImport()}
          disabled={disabled || busy || !url.trim()}
          className="flex h-auto shrink-0 items-center gap-1.5 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-xs font-medium text-foreground/70 transition-colors hover:bg-muted hover:text-foreground/90 disabled:opacity-40"
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
      {!error && imported && (imported.title || imported.company) && (
        <div className="flex items-center gap-1.5 text-[10px] text-brand-soft/90">
          <Check size={10} className="shrink-0" />
          <span className="truncate">
            {t('jobUrlImport.imported')}:{' '}
            {[imported.title, imported.company].filter(Boolean).join(' · ')}
          </span>
        </div>
      )}
    </div>
  );
}
