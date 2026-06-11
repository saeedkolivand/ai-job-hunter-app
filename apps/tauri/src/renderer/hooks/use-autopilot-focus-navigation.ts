import { useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import type { AutopilotFocusEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { keys, useAutopilotFocusEvents, useAutopilotNotificationClick } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * App-global listeners for the autopilot "go look at the finds" signals:
 *  - the shell `autopilot.focus` event (tray "New jobs" click or a validated
 *    deep link) — refreshes the list and, with an `autopilotId`, routes to
 *    /autopilot + flags the card to auto-expand (empty id = refresh only);
 *  - the OS notification click — surfaces the (possibly hidden) window and
 *    focuses the last autopilot via `notificationClicked()`, which re-emits
 *    `autopilot.focus` for that id (handled by `onFocus` above → navigation +
 *    auto-expand). A navigate + invalidate runs as an idempotent
 *    guaranteed-landing fallback.
 *
 * Mounted once in the root layout so they fire regardless of the current route.
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

  const onNotificationClick = useCallback(() => {
    // Surface the window + focus the last autopilot (emits `autopilot.focus` for
    // the specific id → onFocus navigates + auto-expands).
    void api.autopilot.notificationClicked();
    // Idempotent guaranteed-landing fallback in case the focus round-trip is a
    // no-op (e.g. no last autopilot recorded).
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
    void navigate({ to: '/autopilot' });
  }, [api, navigate, qc]);

  useAutopilotFocusEvents(onFocus);
  useAutopilotNotificationClick(onNotificationClick);
}
