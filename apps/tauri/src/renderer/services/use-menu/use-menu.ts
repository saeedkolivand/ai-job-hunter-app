import { useEffect } from 'react';

import type { MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Service-hook wrappers for the native-menu event stream. Mirror the autopilot
 * focus/notification hooks: subscribe on mount, return a sync unsubscribe.
 */
export const useMenuNavigateEvents = (onNavigate?: (event: MenuNavigateEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.menu.onNavigate((event) => {
      onNavigate?.(event);
    });
    return () => off?.();
  }, [api, onNavigate]);
};

export const useMenuActionEvents = (onAction?: (event: MenuActionEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.menu.onAction((event) => {
      onAction?.(event);
    });
    return () => off?.();
  }, [api, onAction]);
};
