/**
 * Unit tests for what `sidepanel.ts` does ONCE, at module load, and on the
 * window's lifecycle: restoring the active tab (storage.session + the Appearance
 * default), the one-shot auto-save notice, theme live-sync (#1236 Half A) and the
 * window-removed session cleanup (#1236 Half B).
 *
 * Each test drives a FRESH `sidepanel.ts` instance via `vi.resetModules()` — the
 * mocked `@wxt-dev/browser` module itself is NOT re-evaluated by that (its
 * `vi.fn()`s and any `mockResolvedValueOnce` queued below survive), only
 * `sidepanel.ts` (and its other, real, non-mocked dependencies) are.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('../../answer-tools/answer-tools', () =>
  import('./test-mocks').then((m) => m.answerToolsMock())
);
vi.mock('../../job-tools/job-tools', () => import('./test-mocks').then((m) => m.jobToolsMock()));
vi.mock('../../job-status/job-status', () => import('./test-mocks').then((m) => m.jobStatusMock()));
vi.mock('../../documents/documents', () => import('./test-mocks').then((m) => m.documentsMock()));
vi.mock('../../prep/prep', () => import('./test-mocks').then((m) => m.prepMock()));
vi.mock('../../lib/site-memory', () => import('./test-mocks').then((m) => m.siteMemoryMock()));
vi.mock('../../connection-status/connection-status', () =>
  import('./test-mocks').then((m) => m.connectionStatusMock())
);
vi.mock('../../lib/answer-state', () => import('./test-mocks').then((m) => m.answerStateMock()));
vi.mock('@wxt-dev/browser', () => import('./test-mocks').then((m) => m.panelBrowserMock()));

import { buildPanelDom, flush, PANEL_WINDOW_ID } from './test-support';

vi.setConfig({ clearMocks: false });

/** Load a fresh `sidepanel.ts` against a fresh panel DOM and let its load chain settle. */
async function freshPanel(): Promise<void> {
  vi.resetModules();
  buildPanelDom();
  await import('../sidepanel');
  await flush();
}

const byId = (id: string) => document.getElementById(id)!;
const tabIsActive = (tab: string): boolean =>
  document.querySelector(`[data-tab="${tab}"]`)!.classList.contains('active');

describe('active tab restore at load (storage.session + Appearance default, items 6 & 11)', () => {
  afterEach(() => {
    vi.mocked(browser.storage.session.get).mockReset().mockResolvedValue({});
    vi.mocked(browser.storage.session.set).mockReset().mockResolvedValue(undefined);
    vi.mocked(browser.storage.local.get).mockReset().mockResolvedValue({});
  });

  it('restores the tab stored in storage.session for this window', async () => {
    vi.mocked(browser.storage.session.get).mockResolvedValueOnce({
      'sidepanelActiveTab:100': 'answers',
    });

    await freshPanel();

    expect(tabIsActive('answers')).toBe(true);
  });

  it('clicking Answers persists sidepanelActiveTab:<windowId> to storage.session', async () => {
    await freshPanel();

    document.querySelector<HTMLButtonElement>('[data-tab="answers"]')!.click();

    expect(browser.storage.session.set).toHaveBeenCalledWith({
      'sidepanelActiveTab:100': 'answers',
    });
  });

  it('falls back to the Appearance default panel tab when nothing is stored for this window', async () => {
    // Call #1 to `local.get` is bootTheme()'s getTheme() (key 'theme',
    // synchronous at the top of the module); call #2 is
    // getDefaultPanelTab()'s own read (key 'defaultPanelTab', only once
    // resolvePanelWindowId()'s chain reaches loadActiveTab()) — a fixed
    // order, so queuing two values in sequence targets each correctly.
    vi.mocked(browser.storage.local.get)
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ defaultPanelTab: 'answers' });

    await freshPanel();

    expect(tabIsActive('answers')).toBe(true);
  });

  it('falls back to job without throwing when storage.session.get rejects', async () => {
    vi.mocked(browser.storage.session.get).mockRejectedValueOnce(new Error('quota'));
    vi.resetModules();
    buildPanelDom();

    await expect(import('../sidepanel')).resolves.toBeDefined();
    await flush();

    expect(tabIsActive('job')).toBe(true);
  });
});

// ── the one-shot save-answers-on-submit auto-save notice (PR4, decision 7) ──
// `checkAutoSaveNotice()` fires unconditionally at module load, ahead of
// every other `sendMessage` call sidepanel.ts makes.

describe('the one-shot save-answers-on-submit auto-save notice (PR4)', () => {
  afterEach(() => {
    vi.mocked(browser.runtime.sendMessage).mockReset();
  });

  /** Queue `text` as the pending notice, then load a fresh panel. */
  const withPendingNotice = (text: string | null) => {
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'autoSaveNotice',
      text,
    });
    return freshPanel();
  };

  it('shows the notice text and reveals the banner when one is pending', async () => {
    await withPendingNotice('Saved 1 answer from this submit — change this in Settings.');

    expect(byId('auto-save-notice').hidden).toBe(false);
    expect(byId('auto-save-notice-text').textContent).toBe(
      'Saved 1 answer from this submit — change this in Settings.'
    );
  });

  it('leaves the banner hidden when nothing is pending', async () => {
    await withPendingNotice(null);

    expect(byId('auto-save-notice').hidden).toBe(true);
  });

  it('the dismiss button hides the banner', async () => {
    await withPendingNotice('Saved 1 answer from this submit.');
    expect(byId('auto-save-notice').hidden).toBe(false);

    byId('auto-save-notice-dismiss').dispatchEvent(new Event('click'));

    expect(byId('auto-save-notice').hidden).toBe(true);
  });
});

// ── theme live-sync (#1236 Half A) ─────────────────────────────────────────
// `subscribeThemeChanges()` runs at panel load in sidepanel.ts (next to
// `bootTheme()`); a `storage.local` change to `theme` must repaint the OPEN
// panel. Only the theme listener is registered here after a fresh import —
// `subscribeAnswerState`/`connection-status` are mocked out — so every
// listener on the mock is the theme one.

describe('theme live-sync (#1236 Half A)', () => {
  afterEach(() => {
    delete document.documentElement.dataset.theme;
  });

  /** Load a fresh panel and return the `storage.onChanged` listeners it registered. */
  async function themeListeners() {
    vi.mocked(browser.storage.onChanged.addListener).mockClear();
    await freshPanel();
    const listeners = vi
      .mocked(browser.storage.onChanged.addListener)
      .mock.calls.map((call) => call[0] as (changes: unknown, areaName: string) => void);
    expect(listeners.length).toBeGreaterThan(0);
    return listeners;
  }

  it('repaints this open panel on a local theme change', async () => {
    for (const listener of await themeListeners()) {
      listener({ theme: { newValue: 'dark' } }, 'local');
    }
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('ignores changes to unrelated keys and other areas while open', async () => {
    const listeners = await themeListeners();
    // A same-area change to an unrelated key must not recalibrate the
    // current theme to system…
    document.documentElement.dataset.theme = 'dark';
    for (const listener of listeners) {
      listener({ defaultPanelTab: { newValue: 'answers' } }, 'local');
    }
    expect(document.documentElement.dataset.theme).toBe('dark');
    // …and the theme key changing in ANOTHER area must not repaint.
    for (const listener of listeners) listener({ theme: { newValue: 'light' } }, 'session');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});

// ── window-removed session cleanup (#1236 Half B) ──────────────────────────
// `storage.session` outlives windows (and window ids get reused), so a
// closed window's `sidepanelActiveTab:<id>` must be cleared when the window
// is removed — otherwise it would mask a newer Appearance default for the
// next window that reuses the id. The user's own explicit choice still wins
// within that window's lifetime (the entry is only removed here).

describe('window-removed session cleanup (#1236 Half B)', () => {
  /** Load a fresh panel, then fire its `windows.onRemoved` listener for `windowId`. */
  async function removeWindow(windowId: number): Promise<void> {
    vi.mocked(browser.storage.session.remove).mockClear();
    vi.mocked(browser.windows.onRemoved.addListener).mockClear();
    await freshPanel();

    const onRemoved = vi.mocked(browser.windows.onRemoved.addListener).mock.calls[0]?.[0];
    if (!onRemoved) throw new Error('windows.onRemoved listener not registered');
    onRemoved(windowId);
  }

  it('clears the remembered active tab for the window that was removed', async () => {
    await removeWindow(PANEL_WINDOW_ID);

    expect(browser.storage.session.remove).toHaveBeenCalledWith('sidepanelActiveTab:100');
  });

  it('leaves a DIFFERENT window key untouched when another window is removed', async () => {
    await removeWindow(999);

    expect(browser.storage.session.remove).toHaveBeenCalledWith('sidepanelActiveTab:999');
    expect(browser.storage.session.remove).not.toHaveBeenCalledWith('sidepanelActiveTab:100');
  });
});
