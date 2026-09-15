import { useCallback } from 'react';
import { useNavigate } from '@tanstack/react-router';

import type { Application, MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { normalizeJobUrl } from '@/features/jobs/lib/canonical-job-key';
import { fetchApplications, useMenuIntents } from '@/services';
import { MANAGED_BY_KEY, useUpdater } from '@/services/use-updater';
import { useWindowControls } from '@/services/use-window-controls';
import { type SettingsSection, useSessionStore } from '@/store/session-store';
import { useUiStore } from '@/store/ui-store';

import type { AppRoute } from './use-keyboard-shortcuts';

/** The two symbolic `menu:navigate` destinations the `ajh://generate?url=`
 *  and `ajh://open?url=` deep links resolve to — see {@link MenuNavigateEvent.route}. */
type JobDeepLinkDestination = 'generate-for-job' | 'open-job';

/** Where a job deep link actually lands, once resolved against the live
 *  Applications list. Exported (pure, no React) for direct unit testing. */
export type JobDeepLinkTarget =
  | { kind: 'application'; id: string; tab?: 'documents' }
  | { kind: 'generate-prefill'; url: string }
  | { kind: 'jobs-search'; url: string };

/**
 * Resolve a `generate-for-job` / `open-job` deep link against the applications
 * already fetched — an Application whose `jobUrl` normalizes to the same
 * canonical identity as the deep link's `url` (mirrors the dedup identity
 * `canonicalJobKey` uses elsewhere, via the same `normalizeJobUrl`).
 *
 * `generate-for-job`: an existing job lands on its Documents tab (the
 * tailor/generate flow); no job lands on a fresh generate session prefilled
 * with the URL (no in-flow generation from a deep link — the pipeline still
 * runs in the renderer, per ADR-050 §PR2 decision 4).
 * `open-job`: an existing job lands on its detail page; no job falls back to
 * the jobs list with the URL as the search term.
 */
export function resolveJobDeepLinkTarget(
  destination: JobDeepLinkDestination,
  url: string,
  applications: Pick<Application, 'id' | 'jobUrl'>[]
): JobDeepLinkTarget {
  const target = normalizeJobUrl(url);
  const match = target ? applications.find((a) => normalizeJobUrl(a.jobUrl) === target) : undefined;

  if (match) {
    return destination === 'generate-for-job'
      ? { kind: 'application', id: match.id, tab: 'documents' }
      : { kind: 'application', id: match.id };
  }
  return destination === 'generate-for-job'
    ? { kind: 'generate-prefill', url }
    : { kind: 'jobs-search', url };
}

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
  'about',
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
  const setJobs = useSessionStore((s) => s.setJobs);
  const setAIGenerate = useSessionStore((s) => s.setAIGenerate);
  const setShortcutsOpen = useUiStore((s) => s.setShortcutsOpen);
  const setExtensionTokenFocus = useUiStore((s) => s.setExtensionTokenFocus);
  const { check } = useUpdater();
  const notify = useNotification();
  const { t } = useTranslation();
  const { isMacos } = useWindowControls();

  // `generate-for-job` / `open-job`: fetch the live applications list (fresh —
  // a cold app has nothing warm yet) and land on whichever page
  // `resolveJobDeepLinkTarget` resolves to. A malformed/missing `url` (should
  // never happen — the shell validates it before dispatch) is a no-op.
  const goToJobDeepLink = useCallback(
    (destination: JobDeepLinkDestination, url: string | undefined) => {
      if (!url) return;
      void fetchApplications().then((applications) => {
        const target = resolveJobDeepLinkTarget(destination, url, applications);
        if (target.kind === 'application') {
          void navigate({
            to: '/applications/$id',
            params: { id: target.id },
            search: target.tab ? { tab: target.tab } : {},
          });
        } else if (target.kind === 'generate-prefill') {
          setAIGenerate({ jobUrl: target.url });
          void navigate({ to: '/ai-generate' });
        } else {
          setJobs({ filter: target.url });
          void navigate({ to: '/jobs' });
        }
      });
    },
    [navigate, setAIGenerate, setJobs]
  );

  const onNavigate = useCallback(
    ({ route, section, focus, url }: MenuNavigateEvent) => {
      if (route === 'generate-for-job' || route === 'open-job') {
        goToJobDeepLink(route, url);
        return;
      }
      if (section && SETTINGS_SECTIONS.includes(section as SettingsSection)) {
        setSettings({ activeSection: section as SettingsSection });
      }
      void navigate({ to: route as AppRoute });
      if (focus === 'extension-token') setExtensionTokenFocus(true);
    },
    [navigate, setSettings, setExtensionTokenFocus, goToJobDeepLink]
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
          } else if (res.managedBy) {
            // A packaged build never checked anything — saying "up to date"
            // here would be a claim we did not make. `by` names the flavour,
            // so a Snap install is never told it came from the Store.
            notify.open({
              key: KEY,
              variant: 'info',
              message: t(MANAGED_BY_KEY[res.managedBy]),
            });
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
  // or hidden — see `useMenuIntents`. The 250 ms poll backstop is macOS-only
  // (NSMenu tracking suppresses focus/visibility/emit on the main window).
  useMenuIntents(onNavigate, onAction, isMacos);
}
