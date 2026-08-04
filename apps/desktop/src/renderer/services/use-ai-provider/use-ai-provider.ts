import {
  useMutation,
  type UseMutationResult,
  useQuery,
  useQueryClient,
  type UseQueryResult,
} from '@tanstack/react-query';

import type { ProviderModelInfo } from '@ajh/shared';

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

/**
 * Explicit return type: TS can't portably "name" the inferred type across the
 * `@ajh/shared` workspace-package boundary (an `isolatedDeclarations`-style
 * requirement, TS2883) once the query result carries `ProviderModelInfo`
 * (added when `listProviderModels` widened past `{name}`) — annotation-only,
 * no behavior change.
 */
export const useListProviderModels = (
  provider: string,
  enabled = true,
  baseUrl?: string
): UseQueryResult<ProviderModelInfo[], Error> => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.models, 'provider-models', provider, baseUrl ?? ''],
    queryFn: () => api.ai.listProviderModels({ provider, baseUrl }),
    enabled: enabled && provider !== 'ollama',
    staleTime: QUERY_TIMES.VERY_LONG,
    // A rejection now means "no key / network error / bad response" — not
    // transient, so retrying just re-pays the backend's own timeout for no
    // gain (see ModelSelector's `modelQueries` for the matching rationale).
    retry: false,
  });
};

/**
 * One-shot provider-model fetch (e.g. to verify a key right after saving it),
 * routed through the service layer rather than calling `api.ai.*` directly.
 * Primes the matching `useListProviderModels` cache on success.
 *
 * Explicit return type for the same TS2883 reason as `useListProviderModels`
 * above — annotation-only, no behavior change.
 */
export const useListProviderModelsLazy = (): UseMutationResult<
  ProviderModelInfo[],
  Error,
  { provider: string; baseUrl?: string }
> => {
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
    // `reembedAll` only returns the job id the instant the job STARTS — the
    // real outcome (reembedded/failed/total) arrives later over `jobs:event`,
    // which `EmbeddingsSettings`'s own listener refetches
    // `keys.ai.embeddingStatus` on (`job.completed`/`job.failed`/
    // `job.cancelled`). An `onSuccess` invalidation here would fire at job
    // START, before a single vector is written — not a safety net for a
    // missed completion event (it can't be, at that timing), just a
    // guaranteed premature refetch of the still-stale status.
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

/** Switch the active generation provider (backend-owned "switch" half).
 *  `setActiveProvider` resolves (never rejects) an `{ error }` union on an invalid
 *  id — narrow it here and throw so React Query's `onError`/the caller's `catch`
 *  fire instead of a false `onSuccess`. */
export const useSetActiveProvider = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (provider: string) => {
      const result = await api.ai.setActiveProvider({ provider });
      if ('error' in result) throw new Error(result.error);
      return result;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/** Edit a provider's model/base_url WITHOUT flipping the active provider (the
 *  backend-owned "edit" half). Same `{ error }`-union narrowing as
 *  `useSetActiveProvider` — a server-side rejection (e.g. base_url provenance)
 *  must reject the mutation, not silently resolve. */
export const useSetProviderSettings = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (req: { provider: string; model?: string; baseUrl?: string }) => {
      const result = await api.ai.setProviderSettings(req);
      if ('error' in result) throw new Error(result.error);
      return result;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/** Set a provider's model (+ optional base_url) AND make it active in one step —
 *  the old Zustand `setAiProviderConfig` full-object semantics used by onboarding.
 *  If `setProviderSettings` rejects (an `{ error }` result), STOP — do not proceed
 *  to `setActiveProvider`, so a rejected save never silently flips the active
 *  provider. */
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
      const settingsResult = await api.ai.setProviderSettings({ provider, model, baseUrl });
      if ('error' in settingsResult) throw new Error(settingsResult.error);
      const activeResult = await api.ai.setActiveProvider({ provider });
      if ('error' in activeResult) throw new Error(activeResult.error);
      return activeResult;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/**
 * Returns the provider/model/baseUrl to inject into every ai_generate call —
 * now backed by the backend `ai_active_config` store (task #16), not Zustand.
 * `effort` STAYS renderer-side (a per-call generation tuning knob read by
 * every reasoning-capable HTTP/CLI provider, not routing/egress).
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
 * Static capabilities of a provider/model (web-search + reasoning-effort
 * support), read straight from the Rust `ModelCapabilities` matrix — never a
 * TS mirror, so a new provider is picked up with zero renderer change. Cheap +
 * rarely-changing, so cached for a long while. Exported so the AI-settings
 * provider-config components can drive the effort picker's visibility from
 * this SAME query, per (provider, model) row — not just the active one.
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
