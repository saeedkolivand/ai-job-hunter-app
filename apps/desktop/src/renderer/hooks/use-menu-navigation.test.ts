import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import type { PendingMenuIntent } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { useMenuNavigation } from './use-menu-navigation';

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
vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector: (s: { setSettings: typeof setSettings }) => unknown) =>
    selector({ setSettings }),
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
 */
function renderWithPending(
  pending: PendingMenuIntent | null,
  takePending = vi.fn().mockResolvedValue(pending)
) {
  const client = createMockClient({ 'menu.takePending': takePending });
  const utils = renderHook(() => useMenuNavigation(), { wrapper: withProviders(client) });
  return { ...utils, takePending };
}

beforeEach(() => {
  navigate.mockClear();
  setSettings.mockClear();
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
});
