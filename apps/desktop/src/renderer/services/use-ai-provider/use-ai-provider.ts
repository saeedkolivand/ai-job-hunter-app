import {
  useMutation,
  type UseMutationResult,
  useQueries,
  useQuery,
  useQueryClient,
  type UseQueryResult,
} from '@tanstack/react-query';

import type { ActiveAiConfig, ProviderModelInfo } from '@ajh/shared';
import type { ModelInspectResult } from '@ajh/shared/schemas';

import { readModelListCache, writeModelListCache } from '@/lib/ai-providers/model-list-cache';
import type { AppClient } from '@/lib/app-client';
import { useAppClient } from '@/providers/AppClientProvider';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig } from '@/store/preferences-store';

import { keys, QUERY_TIMES } from '../query-client';
import { type ProviderSettingsWriteInput, resolveProviderSettingsWrite } from './provider-settings';

/** A cloud provider's model catalogue, plus whether it was served from the
 *  last-good local cache (a live fetch failed) rather than a fresh fetch. */
export interface ProviderModelListResult {
  models: ProviderModelInfo[];
  cached: boolean;
}

/**
 * Fetch a provider's live model catalogue. Two distinct purposes, made
 * explicit at the call site rather than left for a reader to infer:
 *
 * - `allowCacheFallback: true` (DISPLAY, the default) — on failure, fall
 *   back to the last successful list cached locally for that provider + base
 *   URL. Only rejects when BOTH the live fetch fails AND no cache exists —
 *   the picker/settings row can then tell "showing a cached list" apart from
 *   "nothing to show".
 * - `allowCacheFallback: false` (VERIFY) — never falls back. The cache is
 *   keyed by provider + base URL with NO credential identity, so it cannot
 *   prove a *newly entered* key works: a stale list from a previously-saved
 *   (possibly now-revoked or simply different) key would otherwise let a
 *   verification gate — onboarding's Continue, or anything run right after a
 *   key is saved — pass on a request that actually failed. A failed
 *   verification must surface loudly.
 */
export async function fetchProviderModelsWithCache(
  api: AppClient,
  provider: string,
  baseUrl?: string,
  { allowCacheFallback = true }: { allowCacheFallback?: boolean } = {}
): Promise<ProviderModelListResult> {
  try {
    const models = await api.ai.listProviderModels({ provider, baseUrl });
    writeModelListCache(provider, baseUrl, models);
    return { models, cached: false };
  } catch (err) {
    if (!allowCacheFallback) throw err;
    const cached = readModelListCache(provider, baseUrl);
    // `[]` is a valid cached VALUE (a genuinely empty catalogue is a real,
    // once-successful fetch result) but it is not a usable FALLBACK — an
    // array is truthy regardless of length, so `if (cached)` alone would
    // treat "last time this provider had zero models" as license to swallow
    // *this* failure (revoked key / network down / 500) and report it as a
    // cache hit. Require real content before it counts as a fallback.
    if (cached && cached.length > 0) return { models: cached, cached: true };
    throw err;
  }
}

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
      // Capabilities now report whether a search backend is CONFIGURED, not what
      // the provider advertises, so a key change can flip `supportsWebSearch` —
      // and that query is cached with a VERY_LONG staleTime. Without this the
      // research toggle stays wrong until the app restarts.
      void qc.invalidateQueries({ queryKey: keys.ai.capabilities });
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
      // Capabilities now report whether a search backend is CONFIGURED, not what
      // the provider advertises, so a key change can flip `supportsWebSearch` —
      // and that query is cached with a VERY_LONG staleTime. Without this the
      // research toggle stays wrong until the app restarts.
      void qc.invalidateQueries({ queryKey: keys.ai.capabilities });
    },
  });
};

/**
 * Explicit return type: TS can't portably "name" the inferred type across the
 * `@ajh/shared` workspace-package boundary (an `isolatedDeclarations`-style
 * requirement, TS2883) once the query result carries `ProviderModelInfo`
 * (added when `listProviderModels` widened past `{name}`) — annotation-only,
 * no behavior change.
 *
 * `purpose` makes DISPLAY-vs-VERIFY a property of the call, not something a
 * reader has to infer from context:
 * - `'display'` (default) — a stale cache is an honest, disclosed fallback
 *   (the model picker, the Settings row).
 * - `'verify'` — anything gating forward progress on "this key/config
 *   works" (onboarding's Continue). Never serves cache on failure — see
 *   `fetchProviderModelsWithCache`. Also gets its OWN query key (a suffix,
 *   still matched by the existing `['ai','models','provider-models',
 *   provider]` invalidation prefix used elsewhere) so a `'verify'` mount can
 *   never inherit a `'display'` mount's cache-served result for the same
 *   provider + base URL via React Query's own (unrelated) staleTime cache.
 */
export const useListProviderModels = (
  provider: string,
  enabled = true,
  baseUrl?: string,
  purpose: 'display' | 'verify' = 'display'
): UseQueryResult<ProviderModelListResult, Error> => {
  const api = useAppClient();
  return useQuery({
    queryKey:
      purpose === 'verify'
        ? [...keys.ai.models, 'provider-models', provider, baseUrl ?? '', 'verify']
        : [...keys.ai.models, 'provider-models', provider, baseUrl ?? ''],
    queryFn: () =>
      fetchProviderModelsWithCache(api, provider, baseUrl, {
        allowCacheFallback: purpose === 'display',
      }),
    enabled: enabled && provider !== 'ollama',
    staleTime: QUERY_TIMES.VERY_LONG,
    // A rejection now means "no key / network error / bad response" AND (for
    // `'display'`) no cached list to fall back to either — not transient, so
    // retrying just re-pays the backend's own timeout for no gain (see
    // ModelSelector's `modelQueries` for the matching rationale).
    retry: false,
  });
};

/**
 * One-shot provider-model fetch (e.g. to verify a key right after saving it),
 * routed through the service layer rather than calling `api.ai.*` directly.
 * Primes the matching `useListProviderModels` cache on success. Deliberately
 * NOT cache-fallback-aware like `useListProviderModels` — this verifies a
 * freshly saved key, so a failure must surface loudly rather than silently
 * serve a possibly-stale list left over from a different key.
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
      writeModelListCache(provider, baseUrl, models);
      qc.setQueryData([...keys.ai.models, 'provider-models', provider, baseUrl ?? ''], {
        models,
        cached: false,
      } satisfies ProviderModelListResult);
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

/**
 * Inspect SEVERAL local models at once — the advisor's "what is installed and
 * how big is its window" pass.
 *
 * A query rather than the on-demand mutation above: the advisor reads every
 * installed model, `/api/show` is idempotent and its answer is a property of
 * the model file (so it caches for a long time), and `useQueries` is the
 * Rules-of-Hooks-safe way to fan out over a list whose length changes.
 * `null` for a model Ollama can't describe — absent means NOT MEASURED.
 */
export const useModelInspections = (
  models: string[] = []
): { byModel: Record<string, ModelInspectResult | null>; isPending: boolean } => {
  const api = useAppClient();
  // Same guard as `useBoardStatuses`: the service smoke harness calls every
  // exported hook with a single noop argument.
  const safeModels = Array.isArray(models) ? models : [];
  return useQueries({
    queries: safeModels.map((model) => ({
      queryKey: [...keys.ai.models, 'inspect', model],
      queryFn: () => api.ai.inspectModel({ model }),
      staleTime: QUERY_TIMES.VERY_LONG,
      // An unreachable Ollama fails the same way for every model in the list;
      // retrying each one just multiplies the same timeout.
      retry: false,
    })),
    combine: (results) => ({
      byModel: Object.fromEntries(safeModels.map((model, i) => [model, results[i]?.data ?? null])),
      isPending: results.some((r) => r.isPending),
    }),
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

/** Edit a provider's model/base_url/context window WITHOUT flipping the active
 *  provider (the backend-owned "edit" half). Same `{ error }`-union narrowing as
 *  `useSetActiveProvider` — a server-side rejection (e.g. base_url provenance)
 *  must reject the mutation, not silently resolve.
 *
 *  RAW: every field is REPLACED, so a caller that omits one NULLs it. UI call
 *  sites want {@link useSaveProviderSettings}, which fills the fields it isn't
 *  changing; this stays exported for the flows that genuinely own all four. */
export const useSetProviderSettings = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (req: {
      provider: string;
      model?: string;
      baseUrl?: string;
      contextWindow?: number;
    }) => {
      const result = await api.ai.setProviderSettings(req);
      if ('error' in result) throw new Error(result.error);
      return result;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.ai.activeConfig }),
  });
};

/**
 * The REPLACE-safe way to save one provider field.
 *
 * `save({ provider, model })` keeps that provider's stored base URL and the
 * window held for the model; `save({ provider, baseUrl })` keeps its model.
 * Pass `baseUrl: null` to clear a base URL on purpose. Field resolution is the
 * pure {@link resolveProviderSettingsWrite} — the rule lives there, tested,
 * rather than being re-derived at each call site (three of which used to omit
 * enough fields to erase one).
 */
export const useSaveProviderSettings = () => {
  const mutation = useSetProviderSettings();
  const { data: activeConfig } = useActiveConfig();
  const zustand = useAiProviderConfig();
  const localWindows = zustand?.providers?.ollama?.modelLimits;

  const save = (
    input: Omit<ProviderSettingsWriteInput, 'stored' | 'localWindows'>,
    options?: Parameters<typeof mutation.mutate>[1]
  ) =>
    mutation.mutate(
      resolveProviderSettingsWrite({
        ...input,
        stored: activeConfig?.providers?.[input.provider],
        localWindows,
      }),
      options
    );

  return { save, isPending: mutation.isPending };
};

/** Set a provider's model (+ optional base_url) AND make it active in one step —
 *  the old Zustand `setAiProviderConfig` full-object semantics used by onboarding.
 *  If `setProviderSettings` rejects (an `{ error }` result), STOP — do not proceed
 *  to `setActiveProvider`, so a rejected save never silently flips the active
 *  provider. */
export const useConfigureActiveProvider = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const zustand = useAiProviderConfig();
  const localWindows = zustand?.providers?.ollama?.modelLimits;
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
      // Same REPLACE rule as `useSaveProviderSettings`: this flow names the
      // model, so the window it saves is the one held FOR THAT MODEL — never
      // the previous model's, and never silently NULL when one exists.
      const settingsResult = await api.ai.setProviderSettings(
        resolveProviderSettingsWrite({
          provider,
          model,
          baseUrl,
          stored: qc.getQueryData<ActiveAiConfig>(keys.ai.activeConfig)?.providers?.[provider],
          localWindows,
        })
      );
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
 * TS mirror, so a new provider is picked up with zero renderer change.
 *
 * `supportsWebSearch` answers "can research actually run" — a search backend is
 * configured — not "does this provider advertise search". Keyless local Ollama
 * therefore reads false, where it used to read true and return empty briefs.
 * Because that depends on stored keys, the key mutations above invalidate this
 * query; the long staleTime alone would otherwise strand it. Exported so the AI-settings
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
