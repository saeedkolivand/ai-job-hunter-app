import { useEffect, useRef } from 'react';

import { useAppClient } from '@/providers/AppClientProvider';
import { keys, queryClient } from '@/services/query-client';
import { usePreferencesStore } from '@/store/preferences-store';

/**
 * Boot effect for the backend-owned active AI provider store (task #16). Renders
 * nothing; mounted once at the app root.
 *
 *  1. **Prefetch** `ai_active_config` into React Query so the config is warm on
 *     first paint. The imperative prompt-shaping resolver reads it via the
 *     synchronous `queryClient.getQueryData` escape hatch, and the cold-boot
 *     `canRun`/status gates key off it — the prefetch keeps both from reading cold
 *     (a wrong-tier prompt / a flash of "no provider configured").
 *
 *  2. **One-time seed** from the renderer's persisted Zustand `aiProviderConfig`
 *     (the pre-#16 source of truth). Runs ONLY AFTER `persist.hasHydrated()` — a
 *     pre-hydration read would see the default `ollama` and downgrade every
 *     upgrading user (config data-loss). The backend gates the seed on row
 *     presence, so re-calls are safe no-ops and it can never clobber a later
 *     explicit change; a fresh install's onboarding writes (which land after this)
 *     are always preserved. The prefetch is invalidated afterwards so a freshly
 *     seeded config shows immediately.
 */
export function AiConfigBoot() {
  const api = useAppClient();
  const seededRef = useRef(false);

  useEffect(() => {
    // Warm the cache immediately, independent of hydration.
    void queryClient.prefetchQuery({
      queryKey: keys.ai.activeConfig,
      queryFn: () => api.ai.activeConfig(),
    });

    const seed = () => {
      if (seededRef.current) return;
      seededRef.current = true;
      const config = usePreferencesStore.getState().aiProviderConfig;
      // Fresh install (nothing persisted) → let onboarding seed the store via the
      // setters; a seed with an empty snapshot would be a wasted round-trip and
      // must not race the onboarding writes.
      const providerEntries = Object.entries(config?.providers ?? {});
      if (!config?.activeProvider && providerEntries.length === 0) return;

      // Only routing (model + baseUrl) crosses to the backend — the renderer-side
      // tuning knobs (effort, modelLimits) stay in Zustand.
      const providers: Record<string, { model?: string; baseUrl?: string }> = {};
      for (const [id, settings] of providerEntries) {
        providers[id] = { model: settings.model || undefined, baseUrl: settings.baseUrl };
      }
      void api.ai
        .seedActiveConfig({ config: { activeProvider: config?.activeProvider, providers } })
        .then(() => queryClient.invalidateQueries({ queryKey: keys.ai.activeConfig }))
        .catch(() => {});
    };

    if (usePreferencesStore.persist.hasHydrated()) {
      seed();
      return;
    }
    return usePreferencesStore.persist.onFinishHydration(seed);
  }, [api]);

  return null;
}
