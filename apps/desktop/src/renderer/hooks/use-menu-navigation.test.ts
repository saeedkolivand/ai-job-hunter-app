import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import type { PendingMenuIntent } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { resolveJobDeepLinkTarget, useMenuNavigation } from './use-menu-navigation';

// ── Mocks ─────────────────────────────────────────────────────────────────────
// The hook's only side-effect surface is: router navigate, the session/ui store
// setters, and the updater `check`. We mock each so we can assert exact calls.
// Menu intents are delivered by PULLING the shell-buffered intent via
// `menu.takePending` (not by trusting the emitted event payload), so the mock
// client's `takePending` is the unit under test's input.

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
}));

const setSettings = vi.fn();
const setJobs = vi.fn();
const setAIGenerate = vi.fn();
vi.mock('@/store/session-store', () => ({
  useSessionStore: (
    selector: (s: {
      setSettings: typeof setSettings;
      setJobs: typeof setJobs;
      setAIGenerate: typeof setAIGenerate;
    }) => unknown
  ) => selector({ setSettings, setJobs, setAIGenerate }),
}));

const setShortcutsOpen = vi.fn();
const setExtensionTokenFocus = vi.fn();
vi.mock('@/store/ui-store', () => ({
  useUiStore: (
    selector: (s: {
      setShortcutsOpen: typeof setShortcutsOpen;
      setExtensionTokenFocus: typeof setExtensionTokenFocus;
    }) => unknown
  ) => selector({ setShortcutsOpen, setExtensionTokenFocus }),
}));

const check = vi.fn().mockResolvedValue({ available: false });
vi.mock('@/services/use-updater', () => ({
  useUpdater: () => ({ check }),
  MANAGED_BY_KEY: {
    msstore: 'settings.update.managedByStore',
    snap: 'settings.update.managedBySnap',
  },
}));

// useMenuNavigation raises check-for-updates feedback via useNotification and
// reads strings via useTranslation — mock both (provider-free, identity t).
const notifyApi = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};
vi.mock('@ajh/ui', () => ({ useNotification: () => notifyApi }));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Render the hook with a mock client whose `menu.takePending` resolves to
 * `pending` (the shell-buffered intent). On mount the hook drains once; a test
 * can also override `takePending` to script later focus/visibility drains.
 * `clientOverrides` extends the mock client (e.g. `applications.list`) for the
 * job-deep-link tests, which resolve against a fetched applications list.
 */
function renderWithPending(
  pending: PendingMenuIntent | null,
  takePending = vi.fn().mockResolvedValue(pending),
  clientOverrides: Record<string, (...args: never[]) => unknown> = {}
) {
  const client = createMockClient({ 'menu.takePending': takePending, ...clientOverrides });
  const utils = renderHook(() => useMenuNavigation(), { wrapper: withProviders(client) });
  return { ...utils, takePending };
}

beforeEach(() => {
  navigate.mockClear();
  setSettings.mockClear();
  setJobs.mockClear();
  setAIGenerate.mockClear();
  setShortcutsOpen.mockClear();
  setExtensionTokenFocus.mockClear();
  check.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe('useMenuNavigation', () => {
  it('drains a plain navigate intent and routes without touching settings', async () => {
    renderWithPending({ event: 'menu:navigate', payload: { route: '/jobs', section: null } });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/jobs' }));
    expect(setSettings).not.toHaveBeenCalled();
  });

  it('pre-selects an allowlisted settings section then navigates', async () => {
    renderWithPending({ event: 'menu:navigate', payload: { route: '/settings', section: 'ai' } });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/settings' }));
    expect(setSettings).toHaveBeenCalledExactlyOnceWith({ activeSection: 'ai' });
  });

  it('ignores an unknown settings section but still navigates', async () => {
    renderWithPending({
      event: 'menu:navigate',
      payload: { route: '/settings', section: 'bogus' },
    });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/settings' }));
    expect(setSettings).not.toHaveBeenCalled();
  });

  it('triggers the updater check on the check-updates action', async () => {
    renderWithPending({ event: 'menu:action', payload: { action: 'check-updates' } });

    await waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    expect(setShortcutsOpen).not.toHaveBeenCalled();
  });

  // A packaged build's `check` never contacts GitHub — it reports who owns
  // updates. Saying "you are up to date" there would be a claim about a check
  // that did not happen. (`t` is mocked to the identity above, so the
  // assertions are on keys.) Exercised per flavour: a Snap install must
  // never surface the Microsoft Store's own key.
  it.each([
    ['msstore', 'settings.update.managedByStore'],
    ['snap', 'settings.update.managedBySnap'],
  ] as const)('names %s as the update owner instead of claiming "up to date"', async (by, key) => {
    check.mockResolvedValueOnce({ available: false, managedBy: by });
    renderWithPending({ event: 'menu:action', payload: { action: 'check-updates' } });

    await waitFor(() =>
      expect(notifyApi.open).toHaveBeenCalledWith(expect.objectContaining({ message: key }))
    );
    expect(notifyApi.open).not.toHaveBeenCalledWith(
      expect.objectContaining({ message: 'updater.upToDate' })
    );
  });

  it('opens the shortcuts cheat-sheet on the shortcuts action', async () => {
    renderWithPending({ event: 'menu:action', payload: { action: 'shortcuts' } });

    await waitFor(() => expect(setShortcutsOpen).toHaveBeenCalledExactlyOnceWith(true));
    expect(check).not.toHaveBeenCalled();
  });

  it('does nothing when no intent is buffered', async () => {
    const { takePending } = renderWithPending(null);

    await waitFor(() => expect(takePending).toHaveBeenCalled());
    expect(navigate).not.toHaveBeenCalled();
    expect(check).not.toHaveBeenCalled();
    expect(setShortcutsOpen).not.toHaveBeenCalled();
  });

  it('delivers a buffered intent exactly once across multiple triggers', async () => {
    // First drain (mount) returns the intent; every later trigger sees the
    // cleared buffer (atomic take), so navigation fires exactly once.
    const takePending = vi
      .fn()
      .mockResolvedValueOnce({ event: 'menu:navigate', payload: { route: '/jobs', section: null } })
      .mockResolvedValue(null);
    renderWithPending(null, takePending);

    await waitFor(() => expect(navigate).toHaveBeenCalledTimes(1));

    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      document.dispatchEvent(new Event('visibilitychange'));
      await Promise.resolve();
    });

    expect(navigate).toHaveBeenCalledTimes(1);
  });

  it('drains again on window focus (covers the tray/close-to-tray restore)', async () => {
    const takePending = vi.fn().mockResolvedValue(null);
    renderWithPending(null, takePending);

    await waitFor(() => expect(takePending).toHaveBeenCalled());
    expect(navigate).not.toHaveBeenCalled();

    // A later click buffers an intent; the window-focus trigger drains it.
    takePending.mockResolvedValueOnce({
      event: 'menu:navigate',
      payload: { route: '/settings', section: null },
    });
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/settings' }));
  });

  it('sets extensionTokenFocus and still navigates + sets section when focus is extension-token', async () => {
    renderWithPending({
      event: 'menu:navigate',
      payload: { route: '/settings', section: 'accounts', focus: 'extension-token' },
    });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/settings' }));
    expect(setSettings).toHaveBeenCalledExactlyOnceWith({ activeSection: 'accounts' });
    expect(setExtensionTokenFocus).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('does not call setExtensionTokenFocus when focus is absent (native-menu path)', async () => {
    renderWithPending({
      event: 'menu:navigate',
      payload: { route: '/settings', section: 'accounts' },
    });

    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/settings' }));
    expect(setSettings).toHaveBeenCalledExactlyOnceWith({ activeSection: 'accounts' });
    expect(setExtensionTokenFocus).not.toHaveBeenCalled();
  });

  // ── `generate-for-job` / `open-job` deep links (PR2 §B.2) ──────────────────
  // Each fetches the applications list fresh (via `fetchApplications`, backed
  // by the same mock client's `applications.list`) rather than trusting a
  // component-local cache.

  describe('generate-for-job / open-job deep links', () => {
    const URL = 'https://boards.greenhouse.io/acme/jobs/1';

    it('generate-for-job with a matching application lands on its Documents tab', async () => {
      const list = vi.fn().mockResolvedValue([{ id: 'app-1', jobUrl: URL }]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'generate-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() =>
        expect(navigate).toHaveBeenCalledWith({
          to: '/applications/$id',
          params: { id: 'app-1' },
          search: { tab: 'documents' },
        })
      );
      expect(setAIGenerate).not.toHaveBeenCalled();
    });

    it('generate-for-job with no matching application prefills the generate flow with the URL', async () => {
      const list = vi.fn().mockResolvedValue([]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'generate-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/ai-generate' }));
      expect(setAIGenerate).toHaveBeenCalledExactlyOnceWith({ jobUrl: URL });
      expect(setJobs).not.toHaveBeenCalled();
    });

    it('open-job with a matching application lands on its detail page', async () => {
      const list = vi.fn().mockResolvedValue([{ id: 'app-2', jobUrl: URL }]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'open-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() =>
        expect(navigate).toHaveBeenCalledWith({
          to: '/applications/$id',
          params: { id: 'app-2' },
          search: {},
        })
      );
    });

    it('open-job with no matching application falls back to the jobs list, URL as the search term', async () => {
      const list = vi.fn().mockResolvedValue([]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'open-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/jobs' }));
      expect(setJobs).toHaveBeenCalledExactlyOnceWith({ filter: URL });
      expect(setAIGenerate).not.toHaveBeenCalled();
    });

    it('matches an application by canonical URL identity, not exact string equality', async () => {
      // Trailing slash + different casing — same canonical identity as URL.
      const list = vi
        .fn()
        .mockResolvedValue([{ id: 'app-3', jobUrl: 'HTTPS://Boards.Greenhouse.io/acme/jobs/1/' }]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'open-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() =>
        expect(navigate).toHaveBeenCalledWith(
          expect.objectContaining({ to: '/applications/$id', params: { id: 'app-3' } })
        )
      );
    });

    it('does nothing when the deep-link intent carries no url', async () => {
      const list = vi.fn().mockResolvedValue([]);
      const { takePending } = renderWithPending(
        { event: 'menu:navigate', payload: { route: 'open-job', section: null } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(takePending).toHaveBeenCalled());
      expect(list).not.toHaveBeenCalled();
      expect(navigate).not.toHaveBeenCalled();
    });

    // A rejected `applications.list` must not leave the deep link stuck with no
    // navigation and no error handling (an unhandled rejection) — it resolves
    // against an empty list, landing on the same fallback the no-match case uses.
    it('open-job falls back to the jobs list when applications.list rejects', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const list = vi.fn().mockRejectedValue(new Error('offline'));
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'open-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      // React Query's `retry: 1` (query-client.ts) backs off ~1s before the
      // query settles — a longer timeout than the default.
      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/jobs' }), {
        timeout: 3000,
      });
      expect(setJobs).toHaveBeenCalledExactlyOnceWith({ filter: URL });
      expect(consoleError).toHaveBeenCalled();
      consoleError.mockRestore();
    });

    it('generate-for-job falls back to a prefilled generate session when applications.list rejects', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const list = vi.fn().mockRejectedValue(new Error('offline'));
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'generate-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/ai-generate' }), {
        timeout: 3000,
      });
      expect(setAIGenerate).toHaveBeenCalledExactlyOnceWith({ jobUrl: URL });
      expect(consoleError).toHaveBeenCalled();
      consoleError.mockRestore();
    });
  });

  // ── `prep-for-job` deep link (PR4) — the extension side panel's Prep tab's
  // "Prepare in the app" action, same buffered-intent + fetch-applications
  // mechanics as the other two job deep links above.
  describe('prep-for-job deep link', () => {
    const URL = 'https://boards.greenhouse.io/acme/jobs/1';

    it('prep-for-job with a matching application lands on its Interview-prep tab', async () => {
      const list = vi.fn().mockResolvedValue([{ id: 'app-4', jobUrl: URL }]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'prep-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() =>
        expect(navigate).toHaveBeenCalledWith({
          to: '/applications/$id',
          params: { id: 'app-4' },
          search: { tab: 'interview' },
        })
      );
      expect(setAIGenerate).not.toHaveBeenCalled();
    });

    it('prep-for-job with no matching application prefills the generate flow with the URL', async () => {
      const list = vi.fn().mockResolvedValue([]);
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'prep-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/ai-generate' }));
      expect(setAIGenerate).toHaveBeenCalledExactlyOnceWith({ jobUrl: URL });
      expect(setJobs).not.toHaveBeenCalled();
    });

    it('does nothing when the prep-for-job intent carries no url', async () => {
      const list = vi.fn().mockResolvedValue([]);
      const { takePending } = renderWithPending(
        { event: 'menu:navigate', payload: { route: 'prep-for-job', section: null } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(takePending).toHaveBeenCalled());
      expect(list).not.toHaveBeenCalled();
      expect(navigate).not.toHaveBeenCalled();
    });

    it('prep-for-job falls back to a prefilled generate session when applications.list rejects', async () => {
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
      const list = vi.fn().mockRejectedValue(new Error('offline'));
      renderWithPending(
        { event: 'menu:navigate', payload: { route: 'prep-for-job', section: null, url: URL } },
        undefined,
        { 'applications.list': list }
      );

      await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/ai-generate' }), {
        timeout: 3000,
      });
      expect(setAIGenerate).toHaveBeenCalledExactlyOnceWith({ jobUrl: URL });
      expect(consoleError).toHaveBeenCalled();
      consoleError.mockRestore();
    });
  });
});

describe('resolveJobDeepLinkTarget', () => {
  const URL = 'https://boards.greenhouse.io/acme/jobs/1';
  const applications = [{ id: 'app-1', jobUrl: URL }];

  it('routes generate-for-job to the Documents tab of a matching application', () => {
    expect(resolveJobDeepLinkTarget('generate-for-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
      tab: 'documents',
    });
  });

  it('routes open-job to a matching application with no forced tab', () => {
    expect(resolveJobDeepLinkTarget('open-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
    });
  });

  it('falls back to a prefilled generate session when no application matches', () => {
    expect(resolveJobDeepLinkTarget('generate-for-job', URL, [])).toEqual({
      kind: 'generate-prefill',
      url: URL,
    });
  });

  it('falls back to a jobs-list search when no application matches', () => {
    expect(resolveJobDeepLinkTarget('open-job', URL, [])).toEqual({
      kind: 'jobs-search',
      url: URL,
    });
  });

  it('routes prep-for-job to the Interview-prep tab of a matching application', () => {
    expect(resolveJobDeepLinkTarget('prep-for-job', URL, applications)).toEqual({
      kind: 'application',
      id: 'app-1',
      tab: 'interview',
    });
  });

  it('falls back to a prefilled generate session when no application matches prep-for-job', () => {
    expect(resolveJobDeepLinkTarget('prep-for-job', URL, [])).toEqual({
      kind: 'generate-prefill',
      url: URL,
    });
  });

  it('never matches on an unnormalizable url (e.g. a non-http scheme)', () => {
    expect(
      resolveJobDeepLinkTarget('open-job', 'javascript:alert(1)', [
        { id: 'app-1', jobUrl: 'javascript:alert(1)' },
      ])
    ).toEqual({ kind: 'jobs-search', url: 'javascript:alert(1)' });
  });
});
