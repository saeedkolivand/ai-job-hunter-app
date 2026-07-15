import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig } from '@/store/preferences-store';

import { keys, QUERY_TIMES } from '../query-client';

export const useHasProviderKey = (provider: string, enabled = true) => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.models, 'provider-key', provider],
    queryFn: () => api.ai.hasProviderKey({ provider }),
    enabled: enabled && provider !== 'ollama',
    staleTime: QUERY_TIMES.MEDIUM,
  });
};

export const useSetProviderKey = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
      api.ai.setProviderKey({ provider, apiKey }),
    onSuccess: (_data, { provider }) => {
      void qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-key', provider] });
      void qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-models', provider] });
    },
  });
};

export const useRemoveProviderKey = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider }: { provider: string }) => api.ai.removeProviderKey({ provider }),
    onSuccess: (_data, { provider }) => {
      void qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-key', provider] });
    },
  });
};

export const useListProviderModels = (provider: string, enabled = true, baseUrl?: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.models, 'provider-models', provider, baseUrl ?? ''],
    queryFn: () => api.ai.listProviderModels({ provider, baseUrl }),
    enabled: enabled && provider !== 'ollama',
    staleTime: QUERY_TIMES.VERY_LONG,
  });
};

/**
 * One-shot provider-model fetch (e.g. to verify a key right after saving it),
 * routed through the service layer rather than calling `api.ai.*` directly.
 * Primes the matching `useListProviderModels` cache on success.
 */
export const useListProviderModelsLazy = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, baseUrl }: { provider: string; baseUrl?: string }) =>
      api.ai.listProviderModels({ provider, baseUrl }),
    onSuccess: (models, { provider, baseUrl }) => {
      qc.setQueryData([...keys.ai.models, 'provider-models', provider, baseUrl ?? ''], models);
    },
  });
};

export const useTestProviderKey = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: ({ provider, baseUrl }: { provider: string; baseUrl?: string }) =>
      api.ai.testProviderKey({ provider, baseUrl }),
  });
};

/**
 * Inspect a local (Ollama) model's real context window + size via `/api/show`.
 * On-demand (the settings "Analyze model" button), so a mutation rather than a query.
 */
export const useInspectModel = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: ({ model }: { model: string }) => api.ai.inspectModel({ model }),
  });
};

/** Active embedding space, per-space vector counts, and document index coverage. */
export const useEmbeddingStatus = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.embeddingStatus,
    queryFn: () => api.ai.embeddingStatus(),
    staleTime: QUERY_TIMES.SHORT,
  });
};

export const useSetEmbeddingConfig = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { provider: string; model?: string; baseUrl?: string }) =>
      api.ai.setEmbeddingConfig(req),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.ai.embeddingStatus });
    },
  });
};

export const useReembedAll = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: () => api.ai.reembedAll(),
  });
};

/**
 * Today's real AI-spend summary — per-provider token totals as reported by
 * each provider itself (never estimated), plus a best-effort estimated USD
 * cost from a static rate table. Read-only: every write happens server-side,
 * at the point a generation/completion actually runs. Polls on a modest
 * interval (mirrors `useSystemHealth`'s pairing) so the Settings panel keeps
 * up while the user watches a generation/embed finish in the same session —
 * bounded cost since it only polls while the panel is mounted.
 */
export const useSpendSummary = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.spend,
    queryFn: () => api.ai.spendSummary(),
    refetchInterval: QUERY_TIMES.MEDIUM,
    staleTime: QUERY_TIMES.POLLING_STALE,
  });
};

/**
 * The backend-owned active generation config (task #16) — the single source of
 * truth for which provider/model/baseUrl generation routes to. Boot-prefetched
 * (see `AiConfigBoot`) so it is warm on first paint and the synchronous
 * `queryClient.getQueryData(keys.ai.activeConfig)` escape hatch used by the
 * imperative prompt-shaping resolver never reads cold.
 */
export const useActiveConfig = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.activeConfig,
    queryFn: () => api.ai.activeConfig(),
    staleTime: QUERY_TIMES.MEDIUM,
  });
};

/** Switch the active generation provider (backend-owned "switch" half). */
export const useSetActiveProvider = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (provider: string) => api.ai.setActiveProvider({ provider }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/** Edit a provider's model/base_url WITHOUT flipping the active provider (the
 *  backend-owned "edit" half). */
export const useSetProviderSettings = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { provider: string; model?: string; baseUrl?: string }) =>
      api.ai.setProviderSettings(req),
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/** Set a provider's model (+ optional base_url) AND make it active in one step —
 *  the old Zustand `setAiProviderConfig` full-object semantics used by onboarding. */
export const useConfigureActiveProvider = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({
      provider,
      model,
      baseUrl,
    }: {
      provider: string;
      model?: string;
      baseUrl?: string;
    }) => {
      await api.ai.setProviderSettings({ provider, model, baseUrl });
      return api.ai.setActiveProvider({ provider });
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/**
 * Returns the provider/model/baseUrl to inject into every ai_generate call —
 * now backed by the backend `ai_active_config` store (task #16), not Zustand.
 * `effort` STAYS renderer-side (a per-call CLI tuning knob, not routing/egress).
 * `isPending` distinguishes "config not yet loaded" from "resolved but empty" so
 * cold-boot `canRun`/status gates don't flash a false "no provider" state.
 */
export const useGenerateConfig = () => {
  const { data, isPending } = useActiveConfig();
  const zustand = useAiProviderConfig();
  // The backend only ever stores a valid provider id, so narrowing the wire
  // `string` back to `AiProvider` keeps the ~15 downstream consumers' types stable.
  const provider = (data?.activeProvider ?? 'ollama') as AiProvider;
  return {
    provider,
    model: data?.model ?? '',
    baseUrl: data?.baseUrl,
    effort: zustand?.providers?.[provider]?.effort,
    isPending,
  };
};

/**
 * Static capabilities of a provider/model (currently just web-search support),
 * read straight from the Rust `ModelCapabilities` matrix — never a TS mirror, so
 * a new provider is picked up with zero renderer change. Cheap + rarely-changing,
 * so cached for a long while.
 */
export const useModelCapabilities = (provider: string, model: string, baseUrl?: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.capabilities, provider, model, baseUrl ?? ''],
    queryFn: () => api.ai.modelCapabilities({ provider, model, baseUrl }),
    staleTime: QUERY_TIMES.VERY_LONG,
  });
};

/**
 * Capabilities for the ACTIVE provider/model — the single source both tailoring
 * wizards read to default the "search company" toggle ON when the selected model
 * can web-search, OFF otherwise.
 */
export const useActiveModelCapabilities = () => {
  const { provider, model, baseUrl } = useGenerateConfig();
  return useModelCapabilities(provider, model, baseUrl);
};
