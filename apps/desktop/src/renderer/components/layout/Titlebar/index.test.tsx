/**
 * Titlebar — handleTitlebarDoubleClick regression tests
 *
 * Strategy:
 *  - @/services is partially overridden so useWindowControls returns a
 *    controlled toggleMaximize spy — no real Tauri window is needed.
 *  - @tanstack/react-router stubs useRouterState + useNavigate (no RouterProvider).
 *  - @ajh/translations returns keys as-is (global pattern).
 *  - @/store/preferences-store useOnboardingCompleted returns false — suppresses
 *    the title overlay and NotificationBell so the drag region is the only
 *    interactive surface under test.
 *  - @/lib/window-controls-registry onWindowControlsRegistered is a no-op so the
 *    useEffect in Titlebar doesn't fail in jsdom.
 *  - @/lib/parent-route parentRoute returns null (no back-button rendered).
 *  - motion/react is globally shimmed in vitest.setup.ts.
 *
 * Branch matrix covered:
 *  1. mousedown  button:0 detail:2 on bare drag region  → toggleMaximize called once,
 *                                                          preventDefault + stopPropagation invoked.
 *  2. mousedown  button:0 detail:1 (single click)       → toggleMaximize NOT called,
 *                                                          event NOT prevented/stopped.
 *  3. mousedown  button:0 detail:2 inside .app-no-drag  → toggleMaximize NOT called
 *                                                          (closest('.app-no-drag') guard).
 *  4. mouseup    button:0 detail:2 on bare drag region  → toggleMaximize NOT called again
 *                                                          (only mousedown toggles), but
 *                                                          stopPropagation IS invoked.
 *  5. mousedown  button:2 detail:2 (right-click)        → toggleMaximize NOT called
 *                                                          (button !== 0 guard).
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();
const mockHistoryBack = vi.fn();
const routerState = { canGoBack: false };

vi.mock('@tanstack/react-router', () => ({
  useRouterState: ({ select }: { select: (s: { location: { pathname: string } }) => unknown }) =>
    select({ location: { pathname: '/' } }),
  useNavigate: () => mockNavigate,
  useCanGoBack: () => routerState.canGoBack,
  useRouter: () => ({ history: { back: mockHistoryBack } }),
}));

// ── preferences-store — onboarding off so NotificationBell is not mounted ─────

// Mutable container so individual describe blocks can override collapsed state.
const prefsState = {
  sidebarCollapsed: false,
};
const mockToggleSidebar = vi.fn();

vi.mock('@/store/preferences-store', () => ({
  useOnboardingCompleted: () => false,
  useSidebarCollapsed: () => prefsState.sidebarCollapsed,
  useToggleSidebar: () => mockToggleSidebar,
}));

// ── window-controls-registry — no-op so useEffect doesn't fail in jsdom ──────

vi.mock('@/lib/window-controls-registry', () => ({
  onWindowControlsRegistered: vi.fn(),
}));

// ── parent-route — controllable for back-button tests ────────────────────────

const parentRouteState = { value: null as string | null };

vi.mock('@/lib/parent-route', () => ({
  parentRoute: () => parentRouteState.value,
}));

// ── @tauri-apps/plugin-os — return 'windows' so isMac=false in jsdom ─────────

vi.mock('@tauri-apps/plugin-os', () => ({
  platform: () => 'windows',
}));

// ── useWindowControls — controlled spy ────────────────────────────────────────

const mockToggleMaximize = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal<Record<string, unknown>>();
  return {
    ...(orig as object),
    useWindowControls: () => ({ toggleMaximize: mockToggleMaximize }),
  };
});

// ── component under test (import AFTER all vi.mock hoisting) ─────────────────

import { Titlebar } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderTitlebar() {
  return render(<Titlebar />);
}

/** Returns the data-tauri-drag-region div — unique in the rendered tree. */
function dragRegion(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[data-tauri-drag-region]');
  if (!el) throw new Error('data-tauri-drag-region div not found');
  return el;
}

/**
 * Build a synthetic MouseEvent init that matches what the handler inspects.
 * `currentTarget` is set automatically by fireEvent to the dispatched element.
 */
function mouseInit(overrides: Partial<MouseEventInit> = {}): MouseEventInit {
  return { button: 0, detail: 2, bubbles: true, cancelable: true, ...overrides };
}

// ── reset between tests ───────────────────────────────────────────────────────

beforeEach(() => {
  mockToggleMaximize.mockReset();
  mockToggleMaximize.mockResolvedValue(undefined);
  mockToggleSidebar.mockReset();
  mockNavigate.mockReset();
  mockHistoryBack.mockReset();
  prefsState.sidebarCollapsed = false;
  routerState.canGoBack = false;
  parentRouteState.value = null;
});

// ── Branch 1: left double-click mousedown on bare drag region ─────────────────

describe('Titlebar — handleTitlebarDoubleClick', () => {
  it('branch 1 — mousedown button:0 detail:2 on drag region calls toggleMaximize and prevents the event', () => {
    renderTitlebar();
    const region = dragRegion();

    const event = new MouseEvent('mousedown', mouseInit());
    const preventSpy = vi.spyOn(event, 'preventDefault');
    const stopSpy = vi.spyOn(event, 'stopPropagation');

    region.dispatchEvent(event);

    expect(mockToggleMaximize).toHaveBeenCalledOnce();
    expect(preventSpy).toHaveBeenCalledOnce();
    expect(stopSpy).toHaveBeenCalledOnce();
  });

  // ── Branch 2: single click does nothing ─────────────────────────────────────

  it('branch 2 — mousedown button:0 detail:1 (single click) does NOT call toggleMaximize and does NOT prevent the event', () => {
    renderTitlebar();
    const region = dragRegion();

    const event = new MouseEvent('mousedown', mouseInit({ detail: 1 }));
    const preventSpy = vi.spyOn(event, 'preventDefault');
    const stopSpy = vi.spyOn(event, 'stopPropagation');

    region.dispatchEvent(event);

    expect(mockToggleMaximize).not.toHaveBeenCalled();
    expect(preventSpy).not.toHaveBeenCalled();
    expect(stopSpy).not.toHaveBeenCalled();
  });

  // ── Branch 3: target inside .app-no-drag is ignored ─────────────────────────

  it('branch 3 — mousedown detail:2 with target inside .app-no-drag does NOT call toggleMaximize', () => {
    renderTitlebar();

    // The left cluster div carries .app-no-drag; query it and dispatch from there.
    // fireEvent sets target to the dispatched element, so closest('.app-no-drag')
    // will find it and the handler will return early.
    const noDragEl = document.querySelector('.app-no-drag') as HTMLElement;
    expect(noDragEl).not.toBeNull();

    const event = new MouseEvent('mousedown', {
      ...mouseInit(),
      bubbles: true,
      cancelable: true,
    });

    noDragEl.dispatchEvent(event);

    expect(mockToggleMaximize).not.toHaveBeenCalled();
  });

  // ── Branch 4: mouseup double-click suppresses native path but does NOT toggle

  it('branch 4 — mouseup button:0 detail:2 does NOT call toggleMaximize but DOES call stopPropagation', () => {
    renderTitlebar();
    const region = dragRegion();

    const event = new MouseEvent('mouseup', mouseInit());
    const preventSpy = vi.spyOn(event, 'preventDefault');
    const stopSpy = vi.spyOn(event, 'stopPropagation');

    region.dispatchEvent(event);

    // Only mousedown toggles; mouseup must not.
    expect(mockToggleMaximize).not.toHaveBeenCalled();
    // But the event IS prevented/stopped to neutralise macOS's native handler.
    expect(preventSpy).toHaveBeenCalledOnce();
    expect(stopSpy).toHaveBeenCalledOnce();
  });

  // ── Branch 5: non-left button is ignored ────────────────────────────────────

  it('branch 5 — mousedown button:2 detail:2 (right-click) does NOT call toggleMaximize', () => {
    renderTitlebar();
    const region = dragRegion();

    const event = new MouseEvent('mousedown', mouseInit({ button: 2 }));
    const preventSpy = vi.spyOn(event, 'preventDefault');
    const stopSpy = vi.spyOn(event, 'stopPropagation');

    region.dispatchEvent(event);

    expect(mockToggleMaximize).not.toHaveBeenCalled();
    expect(preventSpy).not.toHaveBeenCalled();
    expect(stopSpy).not.toHaveBeenCalled();
  });
});

// ── Collapsed sidebar — expand button ─────────────────────────────────────────

describe('Titlebar — collapsed sidebar', () => {
  it('renders the expand-sidebar button (aria-label=nav.expandSidebar) when sidebar is collapsed', () => {
    prefsState.sidebarCollapsed = true;
    renderTitlebar();
    expect(screen.getByRole('button', { name: 'nav.expandSidebar' })).toBeInTheDocument();
  });

  it('does NOT render the expand-sidebar button when sidebar is expanded', () => {
    prefsState.sidebarCollapsed = false;
    renderTitlebar();
    expect(screen.queryByRole('button', { name: 'nav.expandSidebar' })).not.toBeInTheDocument();
  });

  it('clicking the expand-sidebar button calls the useToggleSidebar spy', async () => {
    prefsState.sidebarCollapsed = true;
    const user = userEvent.setup();
    renderTitlebar();

    await user.click(screen.getByRole('button', { name: 'nav.expandSidebar' }));

    expect(mockToggleSidebar).toHaveBeenCalledOnce();
  });
});

// ── Back button — hybrid visibility + navigation ───────────────────────────────

describe('Titlebar — back button hybrid', () => {
  it('shows back button when canGoBack is true and parent is null (history-only route)', () => {
    routerState.canGoBack = true;
    // parentRouteState.value remains null (no logical parent)
    renderTitlebar();
    expect(screen.getByRole('button', { name: 'nav.back' })).toBeInTheDocument();
  });

  it('clicking back button when parent is set calls navigate({to: parent}), not history.back()', async () => {
    parentRouteState.value = '/applications';
    const user = userEvent.setup();
    renderTitlebar();

    await user.click(screen.getByRole('button', { name: 'nav.back' }));

    expect(mockNavigate).toHaveBeenCalledOnce();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications' });
    expect(mockHistoryBack).not.toHaveBeenCalled();
  });

  it('clicking back button with no parent but canGoBack calls router.history.back()', async () => {
    routerState.canGoBack = true;
    // parentRouteState.value remains null → history path
    const user = userEvent.setup();
    renderTitlebar();

    await user.click(screen.getByRole('button', { name: 'nav.back' }));

    expect(mockHistoryBack).toHaveBeenCalledOnce();
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});
