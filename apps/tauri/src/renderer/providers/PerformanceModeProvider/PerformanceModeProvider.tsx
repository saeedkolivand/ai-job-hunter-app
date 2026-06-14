import { type ReactNode, useEffect, useRef } from 'react';

import { useAppClient } from '@/providers/AppClientProvider';
import { resolveBackendConfig } from '@/store/preferences-schema';
import { usePerformanceMode, useResolvedPerformanceProfile } from '@/store/preferences-store';

/**
 * Syncs the renderer's resolved performance profile to side effects:
 *   1. data-performance-mode  — the mode string (e2e + legacy CSS hooks).
 *   2. data-perf-blur         — the resolved backdrop-blur tier (full/reduced/off).
 *   3. data-perf-animations   — whether aurora/nebula keyframes run (on/off).
 *   4. system.setPerformanceMode IPC — pushes the resolved backend config so the
 *      main process can adjust JobQueue concurrency, model keep-alive, and cache.
 *
 * Place this inside <AppClientProvider> so getClient() is always ready.
 */
export function PerformanceModeProvider({ children }: { children: ReactNode }) {
  const client = useAppClient();
  const mode = usePerformanceMode();
  const profile = useResolvedPerformanceProfile();
  // Last payload pushed over IPC, serialized — re-pushes whenever the resolved
  // config changes (including in-place custom-profile edits), skips redundant IPC.
  const prevPayload = useRef<string | null>(null);

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute('data-performance-mode', mode);
    root.setAttribute('data-perf-blur', profile.visual.blur);
    root.setAttribute('data-perf-animations', profile.visual.animations ? 'on' : 'off');

    // DOM attributes above always reflect the latest profile — cheap, no dedupe.
    // IPC is deduped below: invoking Rust on every visual-only tweak is wasteful.
    const config = resolveBackendConfig(mode, profile);
    const serialized = JSON.stringify(config);
    if (serialized === prevPayload.current) return;

    client.system
      .setPerformanceMode(config)
      .then(() => {
        // Only mark this payload as pushed once the IPC actually succeeds.
        // On failure (e.g. shell not ready on first render) we leave the dedupe
        // cache untouched so the next effect run retries instead of suppressing.
        prevPayload.current = serialized;
      })
      .catch(() => {
        // Silently ignore — main process may not be ready on first render.
      });
  }, [client, mode, profile]);

  return <>{children}</>;
}
