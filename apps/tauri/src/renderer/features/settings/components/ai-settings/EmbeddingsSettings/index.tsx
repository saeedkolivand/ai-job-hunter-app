import { Database, Loader2, RefreshCw } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { JobEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, GlassCard, Input, useNotification } from '@ajh/ui';

import { useEmbeddingStatus, useJobEvents, useReembedAll, useSetEmbeddingConfig } from '@/services';
import { usePreferencesStore, useSemanticScoring } from '@/store/preferences-store';

// Providers that expose an embeddings API. Anthropic is intentionally excluded —
// it has no embeddings endpoint.
const EMBED_PROVIDERS = [
  { value: 'ollama', label: 'Ollama (Local)', defaultModel: 'nomic-embed-text' },
  { value: 'openai', label: 'OpenAI', defaultModel: 'text-embedding-3-small' },
  { value: 'gemini', label: 'Gemini', defaultModel: 'text-embedding-004' },
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
  useJobEvents((evt: JobEvent) => {
    const e = evt as { type: string; jobId: string };
    if (!reindexJobId || e.jobId !== reindexJobId) return;
    if (e.type === 'job.completed' || e.type === 'job.failed' || e.type === 'job.cancelled') {
      setReindexJobId(null);
      void statusQuery.refetch();
      notify.open({
        message:
          e.type === 'job.completed'
            ? t('settings.embeddings.reindexComplete')
            : t('settings.embeddings.reindexIncomplete'),
        variant: e.type === 'job.completed' ? 'success' : 'error',
      });
    }
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
    setReindexJobId(jobId);
    notify.success({ message: t('settings.embeddings.reindexStarted') });
  };

  const docs = status?.documents;
  const stale = docs?.stale ?? 0;
  const reindexing = reindexJobId !== null || reembed.isPending;

  const semanticScoring = useSemanticScoring();
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
            size="sm"
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
        <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-xs">
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
            <div className="mt-1 text-amber-400/80">
              {stale === 1
                ? t('settings.embeddings.staleOne', { count: stale })
                : t('settings.embeddings.staleOther', { count: stale })}
            </div>
          )}
        </div>

        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="sm"
            disabled={reindexing || (docs?.total ?? 0) === 0}
            onClick={() => void onReindex()}
          >
            {reindexing ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
            {reindexing ? t('settings.embeddings.reindexing') : t('settings.embeddings.reindex')}
          </Button>
        </div>

        <div className="flex items-center justify-between rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2.5">
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
