import { useEffect, useRef } from 'react';

import type { ActiveAiConfig } from '@ajh/shared';

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
 *
 *  3. **One-shot context-window backfill** for installs that were already
 *     seeded before the backend row could hold a window. The seed is refused
 *     for them (row presence), so without this their configured `num_ctx`
 *     never reaches a staged run — see `backfillContextWindow` for the guard.
 */
export function AiConfigBoot() {
  const api = useAppClient();
  const seededRef = useRef(false);
  const backfilledRef = useRef(false);

  useEffect(() => {
    // Warm the cache immediately, independent of hydration.
    const prefetched = queryClient.prefetchQuery({
      queryKey: keys.ai.activeConfig,
      queryFn: () => api.ai.activeConfig(),
    });

    /**
     * One-shot backfill for installs that were seeded BEFORE the backend row
     * had a context-window column.
     *
     * The seed above cannot help them: it is row-presence gated server-side, so
     * for an existing install it is refused outright. Their window therefore
     * lives only in Zustand, where no staged run can read it — while the
     * advisor tells them the app "sends the context window you configured with
     * every staged request".
     *
     * The real guard is the DATA, not the ref: this patches only when the
     * backend row has no window and the renderer holds one for that row's
     * model, and the patch itself makes the condition false. The ref only stops
     * a retry storm within one session if the write is rejected. Every other
     * path returns before arming it, so nothing latches on a state it did not
     * act on.
     */
    const backfillContextWindow = async () => {
      if (backfilledRef.current) return;
      const config = queryClient.getQueryData<ActiveAiConfig>(keys.ai.activeConfig);
      const provider = config?.activeProvider;
      // Per-model windows are an Ollama-only preference; no other provider has
      // a local value to promote.
      if (!config || provider !== 'ollama') return;

      const row = config.providers?.[provider];
      const model = row?.model;
      if (!model || row?.contextWindow !== undefined) return;

      const local =
        usePreferencesStore.getState().aiProviderConfig?.providers?.ollama?.modelLimits?.[model]
          ?.contextWindow;
      if (local === undefined) return;

      backfilledRef.current = true;
      // A patch: only the window is sent, so the model and base URL are left
      // exactly as the backend already holds them.
      const result = await api.ai.setProviderSettings({ provider, contextWindow: local });
      if (result && 'error' in result) return;
      await queryClient.invalidateQueries({ queryKey: keys.ai.activeConfig });
    };

    const seed = () => {
      if (seededRef.current) return;
      seededRef.current = true;
      const config = usePreferencesStore.getState().aiProviderConfig;
      // Fresh install (nothing persisted) → let onboarding seed the store via the
      // setters; a seed with an empty snapshot would be a wasted round-trip and
      // must not race the onboarding writes.
      const providerEntries = Object.entries(config?.providers ?? {});
      if (!config?.activeProvider && providerEntries.length === 0) return;

      // Routing (model + baseUrl) plus the window held FOR THAT MODEL — a
      // staged run reads the backend row, so a window left behind in Zustand is
      // a window no staged run can see. The rest of the tuning knobs (effort,
      // maxTokens, temperature) stay renderer-side.
      const providers: Record<
        string,
        { model?: string; baseUrl?: string; contextWindow?: number }
      > = {};
      for (const [id, settings] of providerEntries) {
        const model = settings.model || undefined;
        providers[id] = {
          model,
          baseUrl: settings.baseUrl,
          contextWindow: model ? settings.modelLimits?.[model]?.contextWindow : undefined,
        };
      }
      void api.ai
        .seedActiveConfig({ config: { activeProvider: config?.activeProvider, providers } })
        .then(() => queryClient.invalidateQueries({ queryKey: keys.ai.activeConfig }))
        .catch(() => {});
    };

    // Both run only after hydration: each reads the persisted Zustand config,
    // and a pre-hydration read sees defaults rather than the user's data.
    const boot = () => {
      seed();
      void prefetched.then(backfillContextWindow).catch(() => {});
    };

    if (usePreferencesStore.persist.hasHydrated()) {
      boot();
      return;
    }
    return usePreferencesStore.persist.onFinishHydration(boot);
  }, [api]);

  return null;
}
