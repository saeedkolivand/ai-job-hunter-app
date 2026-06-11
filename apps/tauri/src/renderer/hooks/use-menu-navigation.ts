import { useCallback } from 'react';
import { useNavigate } from '@tanstack/react-router';

import type { MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';

import { useMenuActionEvents, useMenuNavigateEvents } from '@/services';
import { useUpdater } from '@/services/use-updater';
import { type SettingsSection, useSessionStore } from '@/store/session-store';
import { useUiStore } from '@/store/ui-store';

import type { AppRoute } from './use-keyboard-shortcuts';

/** Settings sub-sections we accept off the wire — mirrors the `SettingsSection`
 *  union in session-store. Guards the unchecked cast of an arbitrary string. */
const SETTINGS_SECTIONS: readonly SettingsSection[] = [
  'general',
  'contact',
  'ai',
  'job',
  'resume',
  'accounts',
  'privacy',
  'performance',
  'developer',
];

/**
 * App-global listeners for the native menu (the "richer macOS shell"):
 *  - `menu.navigate` → route to a page, optionally pre-selecting a settings
 *    sub-section (e.g. Settings → AI). Mirrors the autopilot-focus hook.
 *  - `menu.action` → app-level actions that aren't routes: trigger the existing
 *    in-app update check (the `UpdateBanner` surfaces via the shared
 *    `updater:status` event) or open the keyboard-shortcuts cheat-sheet.
 *
 * Mounted once in the root layout so it fires regardless of the current route.
 */
export function useMenuNavigation() {
  const navigate = useNavigate();
  const setSettings = useSessionStore((s) => s.setSettings);
  const setShortcutsOpen = useUiStore((s) => s.setShortcutsOpen);
  const { check } = useUpdater();

  const onNavigate = useCallback(
    ({ route, section }: MenuNavigateEvent) => {
      if (section && SETTINGS_SECTIONS.includes(section as SettingsSection)) {
        setSettings({ activeSection: section as SettingsSection });
      }
      void navigate({ to: route as AppRoute });
    },
    [navigate, setSettings]
  );

  const onAction = useCallback(
    ({ action }: MenuActionEvent) => {
      if (action === 'check-updates') void check();
      else if (action === 'shortcuts') setShortcutsOpen(true);
    },
    [check, setShortcutsOpen]
  );

  useMenuNavigateEvents(onNavigate);
  useMenuActionEvents(onAction);
}
