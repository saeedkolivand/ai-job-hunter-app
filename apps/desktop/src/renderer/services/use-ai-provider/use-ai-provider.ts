import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';
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

/** Returns the provider/model/baseUrl to inject into every ai_generate call. */
export const useGenerateConfig = () => {
  const config = useAiProviderConfig();
  const activeProvider = config?.activeProvider ?? 'ollama';
  const settings = config?.providers?.[activeProvider];
  return {
    provider: activeProvider,
    model: settings?.model ?? '',
    baseUrl: settings?.baseUrl,
    effort: settings?.effort,
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
