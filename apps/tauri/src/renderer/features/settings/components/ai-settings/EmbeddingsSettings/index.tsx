import { Database, Loader2, RefreshCw } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { JobEvent } from '@ajh/shared';
import { Button, Dropdown, GlassCard, Input, useNotification } from '@ajh/ui';

import { useEmbeddingStatus, useJobEvents, useReembedAll, useSetEmbeddingConfig } from '@/services';

// Providers that expose an embeddings API. Anthropic is intentionally excluded —
// it has no embeddings endpoint.
const EMBED_PROVIDERS = [
  { value: 'ollama', label: 'Ollama (Local)', defaultModel: 'nomic-embed-text' },
  { value: 'openai', label: 'OpenAI', defaultModel: 'text-embedding-3-small' },
  { value: 'gemini', label: 'Gemini', defaultModel: 'text-embedding-004' },
  { value: 'openai-compatible', label: 'OpenAI-compatible', defaultModel: '' },
] as const;

export function EmbeddingsSettings() {
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
      notify(
        e.type === 'job.completed' ? 'Re-indexing complete.' : 'Re-indexing did not finish.',
        e.type === 'job.completed' ? 'success' : 'error'
      );
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
      notify('Embedding model updated. Re-index documents to rebuild the index.', 'success');
    } else {
      notify(res.error ?? 'Failed to update the embedding model.', 'error');
    }
  };

  const onReindex = async () => {
    const { jobId } = await reembed.mutateAsync();
    setReindexJobId(jobId);
    notify('Re-indexing documents…', 'success');
  };

  const docs = status?.documents;
  const stale = docs?.stale ?? 0;
  const reindexing = reindexJobId !== null || reembed.isPending;

  return (
    <GlassCard>
      <div className="mb-3 flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
        <Database size={12} /> Embeddings
      </div>
      <p className="mb-3 text-xs text-foreground/40">
        Powers résumé ↔ job matching and semantic search. Each vector is tagged with the model that
        produced it; changing the model rebuilds the index so incompatible vectors are never mixed.
      </p>

      <div className="space-y-3">
        <div className="space-y-1.5">
          <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            Provider
          </div>
          <Dropdown
            options={EMBED_PROVIDERS.map((p) => ({ value: p.value, label: p.label }))}
            value={provider}
            onChange={onProviderChange}
          />
        </div>

        <div className="space-y-1.5">
          <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            Model
          </div>
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={
              EMBED_PROVIDERS.find((p) => p.value === provider)?.defaultModel || 'embedding model'
            }
            className="w-full text-sm"
          />
        </div>

        {provider === 'openai-compatible' && (
          <div className="space-y-1.5">
            <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
              Base URL
            </div>
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com/v1"
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
            {setConfig.isPending ? <Loader2 size={13} className="animate-spin" /> : 'Apply'}
          </Button>
        </div>

        {/* Index status */}
        <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-xs">
          <div className="flex items-center justify-between text-foreground/50">
            <span>
              Active: {status?.active.provider ?? '—'} / {status?.active.model ?? '—'}
            </span>
            {docs && (
              <span>
                {docs.indexedInActiveSpace}/{docs.total} indexed
              </span>
            )}
          </div>
          {stale > 0 && (
            <div className="mt-1 text-amber-400/80">
              {stale} document{stale === 1 ? '' : 's'} need re-indexing for the active model.
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
            {reindexing ? 'Re-indexing…' : 'Re-index documents'}
          </Button>
        </div>
      </div>
    </GlassCard>
  );
}
