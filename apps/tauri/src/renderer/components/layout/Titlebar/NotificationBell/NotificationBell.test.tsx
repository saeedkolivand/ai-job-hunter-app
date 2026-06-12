/**
 * NotificationBell — Priority 2
 *
 * Strategy:
 *  - Service hooks (@/services) are mocked at the module level so the component
 *    reads notifications from a controlled fixture rather than a live query.
 *  - @tanstack/react-router's useRouter is mocked so the component renders without
 *    a RouterProvider.
 *  - motion/react is globally shimmed in vitest.setup.ts — no per-test mock needed.
 *  - @ajh/translations returns keys as-is.
 *  - The ui-store is reset between tests via Zustand setState.
 *
 * Assertions per spec section:
 *  - Badge: shows count of unread items; hidden at 0; shows "9+" for >9.
 *  - Empty list: EmptyState shown.
 *  - Rows: rendered newest-first; unread row shows unread dot.
 *  - Row click: calls markRead + navigates (when route present); no-route row skips navigate.
 *  - Remove button: calls remove, does NOT trigger row navigation (stopPropagation).
 *  - Mark-all-read / Clear-all: call their mutations; mark-all disabled when 0 unread.
 *  - XSS guard: title/body rendered as text, not injected HTML.
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { AppNotification } from '@ajh/shared';

import { useUiStore } from '@/store/ui-store';

import { NotificationBell } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) return `${key}:${JSON.stringify(opts)}`;
      return key;
    },
    i18n: { language: 'en' },
  }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

// ── Service hooks — controlled mock ──────────────────────────────────────────

const mockMarkRead = vi.fn();
const mockMarkAllRead = vi.fn();
const mockRemove = vi.fn();
const mockClearAll = vi.fn();
let mockNotifications: AppNotification[] = [];

vi.mock('@/services', async (importOriginal) => {
  // `importOriginal` returns the real module for pass-through; we spread it so
  // only the notification hooks are overridden, not the whole @/services barrel.
  const orig = await importOriginal();
  return {
    ...(orig as object),
    useNotifications: () => ({ data: mockNotifications }),
    useMarkNotificationRead: () => ({ mutate: mockMarkRead }),
    useMarkAllNotificationsRead: () => ({ mutate: mockMarkAllRead }),
    useRemoveNotification: () => ({ mutate: mockRemove }),
    useClearAllNotifications: () => ({ mutate: mockClearAll }),
  };
});

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeNotification(overrides: Partial<AppNotification>): AppNotification {
  return {
    id: 'n1',
    kind: 'test',
    title: 'Test Title',
    body: 'Test Body',
    createdAt: Date.now(),
    read: false,
    ...overrides,
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderBell() {
  // All service hooks are mocked at module level — the component renders without
  // providers. No IPC is ever invoked.
  return render(<NotificationBell />);
}

function openDropdown() {
  const bellBtn = screen.getByRole('button', { name: /notifications\.bell\.aria/i });
  fireEvent.click(bellBtn);
}

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useUiStore.setState({ notificationsOpen: false });
  mockNotifications = [];
  mockMarkRead.mockReset();
  mockMarkAllRead.mockReset();
  mockRemove.mockReset();
  mockClearAll.mockReset();
  mockNavigate.mockReset();
});

// ── Badge ─────────────────────────────────────────────────────────────────────

describe('NotificationBell — badge', () => {
  it('shows the unread count badge when there are unread notifications', () => {
    mockNotifications = [
      makeNotification({ id: 'n1', read: false }),
      makeNotification({ id: 'n2', read: false }),
      makeNotification({ id: 'n3', read: true }),
    ];
    renderBell();
    // Badge is the span with aria-label containing the count
    const badge = screen.getByLabelText(/notifications\.unread\.aria/i);
    expect(badge).toBeInTheDocument();
    expect(badge.textContent).toBe('2');
  });

  it('hides the badge when all notifications are read', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: true })];
    renderBell();
    expect(screen.queryByLabelText(/notifications\.unread\.aria/i)).not.toBeInTheDocument();
  });

  it('hides the badge when there are no notifications', () => {
    mockNotifications = [];
    renderBell();
    expect(screen.queryByLabelText(/notifications\.unread\.aria/i)).not.toBeInTheDocument();
  });

  it('shows "9+" when unread count exceeds 9', () => {
    mockNotifications = Array.from({ length: 10 }, (_, i) =>
      makeNotification({ id: `n${i}`, read: false })
    );
    renderBell();
    const badge = screen.getByLabelText(/notifications\.unread\.aria/i);
    expect(badge.textContent).toBe('9+');
  });

  it('shows the exact count up to 9', () => {
    mockNotifications = Array.from({ length: 9 }, (_, i) =>
      makeNotification({ id: `n${i}`, read: false })
    );
    renderBell();
    const badge = screen.getByLabelText(/notifications\.unread\.aria/i);
    expect(badge.textContent).toBe('9');
  });
});

// ── Dropdown — empty ─────────────────────────────────────────────────────────

describe('NotificationBell — empty list', () => {
  it('shows EmptyState when the notification list is empty', () => {
    mockNotifications = [];
    renderBell();
    openDropdown();
    // EmptyState renders with the key used in NotificationBell: notifications.empty
    expect(screen.getByText('notifications.empty')).toBeInTheDocument();
  });
});

// ── Dropdown — rows ───────────────────────────────────────────────────────────

describe('NotificationBell — rows', () => {
  it('renders rows newest-first by createdAt', () => {
    mockNotifications = [
      makeNotification({ id: 'n1', title: 'Older', createdAt: 1000 }),
      makeNotification({ id: 'n2', title: 'Newest', createdAt: 3000 }),
      makeNotification({ id: 'n3', title: 'Middle', createdAt: 2000 }),
    ];
    renderBell();
    openDropdown();

    const titles = screen.getAllByText(/Older|Newest|Middle/);
    const textOrder = titles.map((el) => el.textContent);
    expect(textOrder[0]).toBe('Newest');
    expect(textOrder[1]).toBe('Middle');
    expect(textOrder[2]).toBe('Older');
  });

  it('renders an unread dot for unread notifications', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: false })];
    renderBell();
    openDropdown();
    expect(screen.getByLabelText('notifications.unread.dotAria')).toBeInTheDocument();
  });

  it('does NOT render an unread dot for read notifications', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: true })];
    renderBell();
    openDropdown();
    expect(screen.queryByLabelText('notifications.unread.dotAria')).not.toBeInTheDocument();
  });

  it('renders notification title and body as text (XSS guard — no innerHTML injection)', () => {
    const htmlishTitle = '<b>Bold Title</b>';
    const htmlishBody = '<script>evil()</script>';
    mockNotifications = [makeNotification({ id: 'n1', title: htmlishTitle, body: htmlishBody })];
    renderBell();
    openDropdown();

    // The literal string must appear as text, not rendered as HTML.
    expect(screen.getByText(htmlishTitle)).toBeInTheDocument();
    expect(screen.getByText(htmlishBody)).toBeInTheDocument();
    // No <b> or <script> elements should have been injected.
    expect(document.querySelector('b')).not.toBeInTheDocument();
    expect(document.querySelector('script')).not.toBeInTheDocument();
  });
});

// ── Row click — navigation ────────────────────────────────────────────────────

describe('NotificationBell — row click', () => {
  it('clicking a row calls markRead with the notification id', () => {
    mockNotifications = [makeNotification({ id: 'n1', title: 'Click Me' })];
    renderBell();
    openDropdown();

    const row = screen.getByText('Click Me').closest('[role="button"]');
    expect(row).not.toBeNull();
    if (row) fireEvent.click(row);
    expect(mockMarkRead).toHaveBeenCalledWith('n1');
  });

  it('clicking a row with a route calls router.navigate', () => {
    mockNotifications = [
      makeNotification({
        id: 'n1',
        title: 'Navigable',
        route: { to: '/applications', search: { highlight: 'app-1' } },
      }),
    ];
    renderBell();
    openDropdown();

    const row = screen.getByText('Navigable').closest('[role="button"]');
    expect(row).not.toBeNull();
    if (row) fireEvent.click(row);
    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith(expect.objectContaining({ to: '/applications' }));
  });

  it('clicking a row without a route does NOT call router.navigate', () => {
    mockNotifications = [makeNotification({ id: 'n1', title: 'No Route', route: undefined })];
    renderBell();
    openDropdown();

    const row = screen.getByText('No Route').closest('[role="button"]');
    expect(row).not.toBeNull();
    if (row) fireEvent.click(row);
    expect(mockMarkRead).toHaveBeenCalledWith('n1');
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

// ── Remove button ─────────────────────────────────────────────────────────────

describe('NotificationBell — remove button', () => {
  /**
   * HIGH — canonical paired stopPropagation test.
   *
   * A plain row click proves the row handler is live (markRead fires, navigate fires).
   * A remove-button click in the same row proves propagation is actually stopped:
   * remove fires, but markRead and navigate must NOT fire.
   *
   * The previous split-test pattern was a tautology: "markRead not called" also
   * passes if the remove handler itself does nothing. The paired form below would
   * FAIL if stopPropagation were removed from the remove handler.
   */
  it('row click calls markRead+navigate; remove-button click calls remove only (stopPropagation proof)', () => {
    // A notification WITH a route so that a row-click would trigger both markRead and navigate.
    mockNotifications = [
      makeNotification({
        id: 'n1',
        title: 'Paired Test Row',
        route: { to: '/applications', search: { highlight: 'n1' } },
      }),
    ];

    // ── Part (a): plain row click — proves the row handler IS live ────────────
    renderBell();
    openDropdown();

    const row = screen.getByText('Paired Test Row').closest('[role="button"]');
    expect(row).not.toBeNull();
    if (row) fireEvent.click(row);

    expect(mockMarkRead).toHaveBeenCalledWith('n1');
    expect(mockNavigate).toHaveBeenCalledTimes(1);
    expect(mockNavigate).toHaveBeenCalledWith(expect.objectContaining({ to: '/applications' }));

    // Reset call counts between the two halves.
    mockMarkRead.mockReset();
    mockNavigate.mockReset();
    mockRemove.mockReset();

    // Re-open (the row click closed the dropdown via setOpen(false)).
    openDropdown();

    // ── Part (b): remove-button click — proves stopPropagation is actually firing
    const removeBtn = screen.getByRole('button', { name: 'notifications.remove.aria' });
    fireEvent.click(removeBtn);

    expect(mockRemove).toHaveBeenCalledWith('n1');
    // stopPropagation must prevent the row handler from firing.
    expect(mockMarkRead).not.toHaveBeenCalled();
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

// ── Toolbar actions ───────────────────────────────────────────────────────────

describe('NotificationBell — toolbar actions', () => {
  it('mark-all-read button calls markAllRead mutation', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: false })];
    renderBell();
    openDropdown();

    const markAllBtn = screen.getByRole('button', { name: 'notifications.markAllRead' });
    fireEvent.click(markAllBtn);
    expect(mockMarkAllRead).toHaveBeenCalledTimes(1);
  });

  it('mark-all-read button is disabled when there are no unread notifications', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: true })];
    renderBell();
    openDropdown();

    const markAllBtn = screen.getByRole('button', { name: 'notifications.markAllRead' });
    expect(markAllBtn).toBeDisabled();
  });

  it('clear-all button calls clearAll mutation', () => {
    mockNotifications = [makeNotification({ id: 'n1', read: false })];
    renderBell();
    openDropdown();

    const clearAllBtn = screen.getByRole('button', { name: 'notifications.clearAll' });
    fireEvent.click(clearAllBtn);
    expect(mockClearAll).toHaveBeenCalledTimes(1);
  });

  it('clear-all button is disabled when there are no notifications', () => {
    mockNotifications = [];
    renderBell();
    openDropdown();
    // When list is empty the EmptyState shows; toolbar buttons are still rendered in the header.
    const clearAllBtn = screen.getByRole('button', { name: 'notifications.clearAll' });
    expect(clearAllBtn).toBeDisabled();
  });
});

// ── Dropdown close behaviors ──────────────────────────────────────────────────

describe('NotificationBell — dropdown close behaviors', () => {
  /**
   * MEDIUM — untested interaction branch from index.tsx's useEffect:
   * the component attaches document-level mousedown + keydown listeners while
   * the dropdown is open to handle Escape and outside-click closes.
   */
  it('pressing Escape closes the open dropdown', () => {
    mockNotifications = [makeNotification({ id: 'n1', title: 'Open Me' })];
    renderBell();
    openDropdown();

    // Dropdown is open — its content is visible.
    expect(screen.getByText('Open Me')).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });

    // After Escape the dropdown content must be gone.
    expect(screen.queryByText('Open Me')).not.toBeInTheDocument();
  });

  it('a mousedown outside the container closes the open dropdown', () => {
    mockNotifications = [makeNotification({ id: 'n1', title: 'Outside Click Test' })];
    renderBell();
    openDropdown();

    expect(screen.getByText('Outside Click Test')).toBeInTheDocument();

    // Fire mousedown on a node that is outside the NotificationBell container.
    fireEvent.mouseDown(document.body);

    expect(screen.queryByText('Outside Click Test')).not.toBeInTheDocument();
  });

  it('a mousedown INSIDE the container does NOT close the dropdown', () => {
    mockNotifications = [makeNotification({ id: 'n1', title: 'Inside Click Test' })];
    renderBell();
    openDropdown();

    expect(screen.getByText('Inside Click Test')).toBeInTheDocument();

    // Fire mousedown on the notification title text (inside the container).
    const titleEl = screen.getByText('Inside Click Test');
    fireEvent.mouseDown(titleEl);

    // Dropdown must remain open.
    expect(screen.getByText('Inside Click Test')).toBeInTheDocument();
  });
});
