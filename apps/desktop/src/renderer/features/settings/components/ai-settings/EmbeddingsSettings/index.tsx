import { Database, Loader2, RefreshCw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { JobEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, GlassCard, Input, useNotification } from '@ajh/ui';

import { useEmbeddingStatus, useJobEvents, useReembedAll, useSetEmbeddingConfig } from '@/services';
import {
  useAutoIndexOnUpload,
  usePreferencesStore,
  useSemanticScoring,
} from '@/store/preferences-store';

// Providers that expose an embeddings API. Anthropic is intentionally excluded —
// it has no embeddings endpoint.
const EMBED_PROVIDERS = [
  { value: 'ollama', label: 'Ollama (Local)', defaultModel: 'nomic-embed-text' },
  { value: 'openai', label: 'OpenAI', defaultModel: 'text-embedding-3-small' },
  { value: 'gemini', label: 'Gemini', defaultModel: 'gemini-embedding-2' },
  { value: 'openai-compatible', label: 'OpenAI-compatible', defaultModel: '' },
] as const;

export function EmbeddingsSettings() {
  const { t } = useTranslation();
  const notify = useNotification();
  const statusQuery = useEmbeddingStatus();
  const status = statusQuery.data;
  const setConfig = useSetEmbeddingConfig();
  const reembed = useReembedAll();

  const [provider, setProvider] = useState('ollama');
  const [model, setModel] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [reindexJobId, setReindexJobId] = useState<string | null>(null);
  // `setReindexJobId` schedules a render — until it commits, any closure
  // registered BEFORE this update (e.g. `useJobEvents`'s callback from the
  // prior render) still reads `reindexJobId` as `null`. A fast terminal
  // event racing ahead of that commit would then be silently dropped (jobId
  // mismatch against `null`), leaving the panel stuck "reindexing" forever
  // — nothing else ever clears the tracked id or refetches. A ref is a
  // single mutable box shared by every closure regardless of which render
  // captured it, so writing it FIRST (synchronously, before the state
  // update) closes that window: even a stale-closure handler invocation
  // still reads the just-written value.
  const reindexJobIdRef = useRef<string | null>(null);

  // Mirror the persisted active config into the form once it loads / changes.
  const activeProvider = status?.active.provider;
  const activeModel = status?.active.model;
  const activeBaseUrl = status?.active.baseUrl;
  useEffect(() => {
    if (activeProvider) {
      setProvider(activeProvider);
      setModel(activeModel ?? '');
      setBaseUrl(activeBaseUrl ?? '');
    }
  }, [activeProvider, activeModel, activeBaseUrl]);

  // Watch the re-index job to surface completion and refresh the status panel.
  // `job.completed`'s data is `{ reembedded, failed, total }`; `job.failed`'s
  // data is the first provider error string (see `commands::ai::ai_reembed_all`).
  // A `failed > 0` completed run is a PARTIAL failure (some documents wrote,
  // some didn't) — it must never read as the same flat "success" as a clean run.
  useJobEvents((evt: JobEvent) => {
    const e = evt as { type: string; jobId: string; data?: unknown };
    if (!reindexJobIdRef.current || e.jobId !== reindexJobIdRef.current) return;
    if (e.type !== 'job.completed' && e.type !== 'job.failed' && e.type !== 'job.cancelled') return;

    reindexJobIdRef.current = null;
    setReindexJobId(null);
    void statusQuery.refetch();

    if (e.type === 'job.failed') {
      const reason = typeof e.data === 'string' ? e.data : undefined;
      notify.error({
        message: reason
          ? t('settings.embeddings.reindexFailedReason', { reason })
          : t('settings.embeddings.reindexIncomplete'),
      });
      return;
    }

    if (e.type === 'job.cancelled') {
      // A user-initiated cancel isn't a failure — `warning` (the index is
      // still stale, worth noting) reads better than `error`.
      notify.warning({ message: t('settings.embeddings.reindexIncomplete') });
      return;
    }

    const data = e.data as { failed?: number; total?: number } | undefined;
    const failedCount = data?.failed ?? 0;
    if (failedCount > 0) {
      notify.warning({
        message: t('settings.embeddings.reindexPartial', {
          failed: failedCount,
          total: data?.total ?? 0,
        }),
      });
      return;
    }

    notify.success({ message: t('settings.embeddings.reindexComplete') });
  });

  const onProviderChange = (p: string) => {
    setProvider(p);
    setModel(EMBED_PROVIDERS.find((x) => x.value === p)?.defaultModel ?? '');
  };

  const dirty =
    !!status &&
    (status.active.provider !== provider ||
      status.active.model !==
        (model.trim() || EMBED_PROVIDERS.find((x) => x.value === provider)?.defaultModel || '') ||
      (provider === 'openai-compatible' && (status.active.baseUrl ?? '') !== baseUrl.trim()));

  const onApply = async () => {
    const res = await setConfig.mutateAsync({
      provider,
      model: model.trim() || undefined,
      baseUrl: provider === 'openai-compatible' ? baseUrl.trim() || undefined : undefined,
    });
    if (res.success) {
      notify.success({ message: t('settings.embeddings.applied') });
    } else {
      notify.error({ message: res.error ?? t('settings.embeddings.applyFailed') });
    }
  };

  const onReindex = async () => {
    const { jobId } = await reembed.mutateAsync();
    reindexJobIdRef.current = jobId;
    setReindexJobId(jobId);
    // `info`, not `success` — "started" is not "succeeded"; the job-event
    // handler above is the only place that reports the real outcome.
    notify.info({ message: t('settings.embeddings.reindexStarted') });
  };

  const docs = status?.documents;
  const stale = docs?.stale ?? 0;
  const reindexing = reindexJobId !== null || reembed.isPending;

  const semanticScoring = useSemanticScoring();
  const autoIndexOnUpload = useAutoIndexOnUpload();
  const setAutoIndexOnUpload = usePreferencesStore((s) => s.setAutoIndexOnUpload);
  const setSemanticScoring = usePreferencesStore((s) => s.setSemanticScoring);

  return (
    <GlassCard>
      <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        <Database size={12} /> {t('settings.embeddings.heading')}
      </div>
      <p className="mb-3 text-xs text-foreground/40">{t('settings.embeddings.description')}</p>

      <div className="space-y-3">
        <div className="space-y-1.5">
          <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.embeddings.provider')}
          </div>
          <Dropdown
            options={EMBED_PROVIDERS.map((p) => ({ value: p.value, label: p.label }))}
            value={provider}
            onChange={onProviderChange}
          />
        </div>

        {provider !== 'ollama' && (
          <p className="text-[11px] text-amber-400/80">
            {t('settings.embeddings.cloudCostAdvisory')}{' '}
            <code className="text-foreground/70">nomic-embed-text</code>.
          </p>
        )}

        <div className="space-y-1.5">
          <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.embeddings.model')}
          </div>
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={
              EMBED_PROVIDERS.find((p) => p.value === provider)?.defaultModel ||
              t('settings.embeddings.modelPlaceholder')
            }
            className="w-full text-sm"
          />
        </div>

        {provider === 'openai-compatible' && (
          <div className="space-y-1.5">
            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
              {t('settings.embeddings.baseUrl')}
            </div>
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder={t('settings.embeddings.baseUrlPlaceholder')}
              className="w-full text-sm"
            />
          </div>
        )}

        <div className="flex justify-end">
          <Button
            variant="glass"
            disabled={!dirty || setConfig.isPending}
            onClick={() => void onApply()}
          >
            {setConfig.isPending ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              t('settings.embeddings.apply')
            )}
          </Button>
        </div>

        {/* Index status */}
        <div className="rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2 text-xs">
          <div className="flex items-center justify-between text-foreground/50">
            <span>
              {t('settings.embeddings.activeLabel')} {status?.active.provider ?? '—'} /{' '}
              {status?.active.model ?? '—'}
            </span>
            {docs && (
              <span>
                {t('settings.embeddings.indexed', {
                  indexed: docs.indexedInActiveSpace,
                  total: docs.total,
                })}
              </span>
            )}
          </div>
          {stale > 0 && (
            // Informational, not a warning: an unindexed document blocks
            // nothing — `match_resume` embeds it the first time it needs a
            // vector. The old amber "needs re-indexing" line read as a chore.
            <div className="mt-1 text-foreground/40">
              {autoIndexOnUpload
                ? stale === 1
                  ? t('settings.embeddings.staleAutoOne', { count: stale })
                  : t('settings.embeddings.staleAutoOther', { count: stale })
                : stale === 1
                  ? t('settings.embeddings.staleOne', { count: stale })
                  : t('settings.embeddings.staleOther', { count: stale })}
            </div>
          )}
        </div>

        <div className="flex justify-end">
          <Button
            variant="ghost"
            disabled={reindexing || (docs?.total ?? 0) === 0}
            onClick={() => void onReindex()}
          >
            {reindexing ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
            {reindexing ? t('settings.embeddings.reindexing') : t('settings.embeddings.reindex')}
          </Button>
        </div>

        <div className="flex items-center justify-between rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2.5">
          <div className="space-y-0.5">
            <p className="text-xs font-semibold text-foreground/70">
              {t('settings.embeddings.autoIndex')}
            </p>
            <p className="text-[11px] text-foreground/40 leading-relaxed">
              {t('settings.embeddings.autoIndexDesc')}
            </p>
          </div>
          <input
            type="checkbox"
            checked={autoIndexOnUpload}
            onChange={(e) => setAutoIndexOnUpload(e.target.checked)}
            className="h-4 w-4 accent-[var(--color-brand)] cursor-pointer"
            aria-label={t('settings.embeddings.autoIndex')}
          />
        </div>

        <div className="flex items-center justify-between rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2.5">
          <div className="space-y-0.5">
            <p className="text-xs font-semibold text-foreground/70">
              {t('settings.embeddings.semanticScoring')}
            </p>
            <p className="text-[11px] text-foreground/40 leading-relaxed">
              {t('settings.embeddings.semanticScoringDesc')}
            </p>
          </div>
          <input
            type="checkbox"
            checked={semanticScoring}
            onChange={(e) => setSemanticScoring(e.target.checked)}
            className="h-4 w-4 accent-[var(--color-brand)] cursor-pointer"
          />
        </div>
      </div>
    </GlassCard>
  );
}
