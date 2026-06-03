import { useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import type { AutopilotFocusEvent } from '@ajh/shared';

import { keys, useAutopilotFocusEvents, useAutopilotNotificationClick } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * App-global listeners for the autopilot "go look at the finds" signals:
 *  - the shell `autopilot.focus` event (tray "New jobs" click or a validated
 *    deep link) — refreshes the list and, with an `autopilotId`, routes to
 *    /autopilot + flags the card to auto-expand (empty id = refresh only);
 *  - the OS notification click — opens the autopilot page (no specific card,
 *    since the notification carries no id; the run's card shows a New badge).
 *
 * Mounted once in the root layout so they fire regardless of the current route.
 */
export function useAutopilotFocusNavigation() {
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
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
    void navigate({ to: '/autopilot' });
  }, [navigate, qc]);

  useAutopilotFocusEvents(onFocus);
  useAutopilotNotificationClick(onNotificationClick);
}
