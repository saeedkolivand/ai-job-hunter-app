import { useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import type { AutopilotFocusEvent } from '@ajh/shared';

import { keys, useAutopilotFocusEvents } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * App-global listener for the shell's `autopilot.focus` event (raised by a tray
 * "New jobs" click or a validated deep link). Always refreshes the autopilot
 * list so tray-side changes (e.g. Pause-All) reflect; when an `autopilotId` is
 * present, it also routes to /autopilot and flags the card to auto-expand its
 * found-jobs. An empty id is a pure refresh signal (no navigation).
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
