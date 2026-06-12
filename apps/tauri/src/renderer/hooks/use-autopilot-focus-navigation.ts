import { useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import type { AutopilotFocusEvent } from '@ajh/shared';

import { keys, useAutopilotFocusEvents } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * App-global listener for the autopilot "go look at the finds" signal: the shell
 * `autopilot:focus` event (tray "New jobs" click or a validated deep link) —
 * refreshes the list and, with an `autopilotId`, routes to /autopilot + flags the
 * card to auto-expand (empty id = refresh only).
 *
 * Mounted once in the root layout so it fires regardless of the current route.
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

  useAutopilotFocusEvents(onFocus);
}
