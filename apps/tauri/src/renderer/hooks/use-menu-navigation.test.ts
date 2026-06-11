import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { useMenuNavigation } from './use-menu-navigation';

// ── Mocks ─────────────────────────────────────────────────────────────────────
// The hook's only side-effect surface is: router navigate, the session/ui store
// setters, and the updater `check`. We mock each so we can assert exact calls.
// The menu subscription itself stays real — we capture the handler it registers
// through the mock AppClient's `menu.onNavigate` / `menu.onAction`.

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigate,
}));

const setSettings = vi.fn();
vi.mock('@/store/session-store', () => ({
  // Selector-based store: invoke the selector against a state exposing the spy.
  useSessionStore: (selector: (s: { setSettings: typeof setSettings }) => unknown) =>
    selector({ setSettings }),
}));

const setShortcutsOpen = vi.fn();
vi.mock('@/store/ui-store', () => ({
  useUiStore: (selector: (s: { setShortcutsOpen: typeof setShortcutsOpen }) => unknown) =>
    selector({ setShortcutsOpen }),
}));

const check = vi.fn();
vi.mock('@/services/use-updater', () => ({
  useUpdater: () => ({ check }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Render the hook with a mock client whose menu subscriptions capture the
 * registered handlers, so a test can fire menu events directly.
 */
function renderWithCapture() {
  let onNavigate: ((e: MenuNavigateEvent) => void) | undefined;
  let onAction: ((e: MenuActionEvent) => void) | undefined;

  const client = createMockClient({
    'menu.onNavigate': vi.fn((handler: (e: MenuNavigateEvent) => void) => {
      onNavigate = handler;
      return () => {};
    }),
    'menu.onAction': vi.fn((handler: (e: MenuActionEvent) => void) => {
      onAction = handler;
      return () => {};
    }),
  });

  const utils = renderHook(() => useMenuNavigation(), { wrapper: withProviders(client) });

  return {
    ...utils,
    fireNavigate: (e: MenuNavigateEvent) => act(() => onNavigate?.(e)),
    fireAction: (e: MenuActionEvent) => act(() => onAction?.(e)),
  };
}

beforeEach(() => {
  navigate.mockClear();
  setSettings.mockClear();
  setShortcutsOpen.mockClear();
  check.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe('useMenuNavigation', () => {
  it('routes a plain navigate event without touching settings', () => {
    const { fireNavigate } = renderWithCapture();

    fireNavigate({ route: '/jobs', section: null });

    expect(navigate).toHaveBeenCalledExactlyOnceWith({ to: '/jobs' });
    expect(setSettings).not.toHaveBeenCalled();
  });

  it('pre-selects an allowlisted settings section then navigates', () => {
    const { fireNavigate } = renderWithCapture();

    fireNavigate({ route: '/settings', section: 'ai' });

    expect(setSettings).toHaveBeenCalledExactlyOnceWith({ activeSection: 'ai' });
    expect(navigate).toHaveBeenCalledExactlyOnceWith({ to: '/settings' });
  });

  it('ignores an unknown settings section but still navigates', () => {
    const { fireNavigate } = renderWithCapture();

    fireNavigate({ route: '/settings', section: 'bogus' });

    expect(setSettings).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledExactlyOnceWith({ to: '/settings' });
  });

  it('triggers the updater check on the check-updates action', () => {
    const { fireAction } = renderWithCapture();

    fireAction({ action: 'check-updates' });

    expect(check).toHaveBeenCalledTimes(1);
    expect(setShortcutsOpen).not.toHaveBeenCalled();
  });

  it('opens the shortcuts cheat-sheet on the shortcuts action', () => {
    const { fireAction } = renderWithCapture();

    fireAction({ action: 'shortcuts' });

    expect(setShortcutsOpen).toHaveBeenCalledExactlyOnceWith(true);
    expect(check).not.toHaveBeenCalled();
  });
});
