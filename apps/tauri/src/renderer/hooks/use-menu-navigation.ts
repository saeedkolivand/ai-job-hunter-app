import { useCallback } from 'react';
import { useNavigate } from '@tanstack/react-router';

import type { MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useMenuIntents } from '@/services';
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
 *  - `menu:navigate` → route to a page, optionally pre-selecting a settings
 *    sub-section (e.g. Settings → AI). Mirrors the autopilot-focus hook.
 *  - `menu:action` → app-level actions that aren't routes: trigger the existing
 *    in-app update check (the `UpdateBanner` surfaces via the shared
 *    `updater:status` event) or open the keyboard-shortcuts cheat-sheet.
 *
 * Mounted once in the root layout so it fires regardless of the current route.
 */
export function useMenuNavigation() {
  const navigate = useNavigate();
  const setSettings = useSessionStore((s) => s.setSettings);
  const setShortcutsOpen = useUiStore((s) => s.setShortcutsOpen);
  const setExtensionTokenFocus = useUiStore((s) => s.setExtensionTokenFocus);
  const { check } = useUpdater();
  const notify = useNotification();
  const { t } = useTranslation();

  const onNavigate = useCallback(
    ({ route, section, focus }: MenuNavigateEvent) => {
      if (section && SETTINGS_SECTIONS.includes(section as SettingsSection)) {
        setSettings({ activeSection: section as SettingsSection });
      }
      void navigate({ to: route as AppRoute });
      if (focus === 'extension-token') setExtensionTokenFocus(true);
    },
    [navigate, setSettings, setExtensionTokenFocus]
  );

  const onAction = useCallback(
    ({ action }: MenuActionEvent) => {
      if (action === 'shortcuts') {
        setShortcutsOpen(true);
        return;
      }
      if (action !== 'check-updates') return;
      // A manual check needs explicit feedback: the UpdateBanner only surfaces an
      // *available* update, so without this an up-to-date / errored check looks
      // like nothing happened. We reuse one notification (by key) — "checking…"
      // then the outcome. An available update hands off to the banner.
      const KEY = 'update-check';
      void (async () => {
        notify.open({ key: KEY, variant: 'info', duration: 0, message: t('updater.checking') });
        try {
          const res = await check();
          if ('error' in res) {
            notify.open({ key: KEY, variant: 'error', message: res.error });
          } else if (res.available) {
            notify.destroy(KEY); // the UpdateBanner takes over
          } else {
            notify.open({ key: KEY, variant: 'success', message: t('updater.upToDate') });
          }
        } catch (e) {
          notify.open({
            key: KEY,
            variant: 'error',
            message: e instanceof Error ? e.message : t('updater.checkFailed'),
          });
        }
      })();
    },
    [check, notify, setShortcutsOpen, t]
  );

  // Single reliable delivery path: the shell buffers the intent and we pull it
  // (on the emitted event, on window focus/visibility-restore, and on mount).
  // Works from the tray and the macOS menu bar, whether the window was visible
  // or hidden — see `useMenuIntents`.
  useMenuIntents(onNavigate, onAction);
}
