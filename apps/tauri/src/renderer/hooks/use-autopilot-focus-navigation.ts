import { useCallback, useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import type { AutopilotFocusEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { keys, useAutopilotFocusEvents } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * App-global listener for the autopilot "go look at the finds" signal: the shell
 * `autopilot:focus` event (tray "New jobs" click or a validated deep link) —
 * refreshes the list and, with an `autopilotId`, routes to /autopilot + flags the
 * card to auto-expand (empty id = refresh only).
 *
 * Mounted once in the root layout so it fires regardless of the current route.
 *
 * Delivery model (mirrors use-menu):
 *  - Live `autopilot:focus` event — low-latency for already-running windows.
 *  - Pull via `autopilot.takePendingFocus()` on mount + window focus +
 *    visibility-restore — covers cold-start deep links where the Rust setup emits
 *    before JS listeners are attached. The shell buffers the id atomically so the
 *    event + pull can't double-fire (take-and-clear).
 */
export function useAutopilotFocusNavigation() {
  const api = useAppClient();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const setAutopilot = useSessionStore((s) => s.setAutopilot);

  const onFocus = useCallback(
    (event: AutopilotFocusEvent) => {
      void qc.invalidateQueries({ queryKey: keys.autopilot.all });
      const id = event.autopilotId?.trim();
      if (!id) return;
      setAutopilot({ focusedId: id });
      void navigate({ to: '/autopilot' });
    },
    [navigate, qc, setAutopilot]
  );

  // Live event listener — already-running window path.
  useAutopilotFocusEvents(onFocus);

  // Pull drain — cold-start deep-link path + tray-restore path.
  // Mirrors use-menu's drain triggers: mount, window focus, visibility-restore.
  useEffect(() => {
    let cancelled = false;
    const drain = async () => {
      try {
        const id = await api.autopilot.takePendingFocus();
        if (cancelled || !id) return;
        onFocus({ autopilotId: id });
      } catch {
        // Transient IPC failure — the buffer is retained on the shell side;
        // a later trigger re-drains. Never surface as an unhandled rejection.
      }
    };
    const trigger = () => void drain();
    const onVisibility = () => {
      if (document.visibilityState === 'visible') void drain();
    };

    void drain(); // mount: drain anything buffered before listeners attached
    window.addEventListener('focus', trigger);
    document.addEventListener('visibilitychange', onVisibility);

    return () => {
      cancelled = true;
      window.removeEventListener('focus', trigger);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  }, [api, onFocus]);
}
