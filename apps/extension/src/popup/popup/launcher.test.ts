/**
 * The popup launcher's own wired-DOM behavior (PR0 §2): the "?" menu, the notice
 * line, "Open the panel →", the one-shot auto-save notice, and theme live-sync.
 * The interactive Answer-tools UI itself lives in the side panel's Answers tab.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', async () => (await import('./browser-mock')).popupBrowserMock());
vi.mock('../../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import { bootPopup, byId, flush } from './test-support';

const { bootstrapNotice, checkAutoSaveNotice } = await bootPopup();
const sendMessageMock = vi.mocked(browser.runtime.sendMessage);

const hidden = (id: string): boolean => byId<HTMLElement>(id).hidden;
const textOf = (id: string): string | null => byId<HTMLElement>(id).textContent;
const click = (id: string): void => byId<HTMLButtonElement>(id).click();

describe('the "?" menu (#btn-help → Help center / Settings / About)', () => {
  beforeEach(() => {
    byId<HTMLElement>('menu').hidden = true;
    byId<HTMLElement>('help-popover').hidden = true;
    byId<HTMLElement>('about-popover').hidden = true;
    byId<HTMLButtonElement>('btn-help').setAttribute('aria-expanded', 'false');
  });

  it('opens the menu on click, closes it on a second click', () => {
    const btn = byId<HTMLButtonElement>('btn-help');

    btn.click();
    expect(hidden('menu')).toBe(false);
    expect(btn.getAttribute('aria-expanded')).toBe('true');

    btn.click();
    expect(hidden('menu')).toBe(true);
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });

  it('"Help center" swaps the menu for the existing help-popover content', () => {
    click('btn-help');
    click('menu-help');

    expect(hidden('menu')).toBe(true);
    expect(hidden('help-popover')).toBe(false);
  });

  it('"Settings" opens the options page and closes the menu', () => {
    click('btn-help');
    click('menu-settings');

    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
    expect(hidden('menu')).toBe(true);
  });

  it('"About" shows the version line from the manifest', () => {
    click('btn-help');
    click('menu-about');

    expect(hidden('about-popover')).toBe(false);
    expect(textOf('about-version')).toContain('1.2.3');
  });
});

// ── bootstrapNotice (resolves the active tab, subscribes the notice line) ────
// The interactive rows live only in the panel's Answers tab — this popup only
// reports a passive count.

describe('bootstrapNotice', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLElement>('answers-notice').hidden = true;
    byId<HTMLElement>('answers-notice').textContent = '';
  });

  it('subscribes the notice line to the shared answer state instead of querying for it', async () => {
    // The stream lives in the shared per-tab state (ADR-044 decision 1) — a
    // query would go stale the moment the panel changed it, which subscribing
    // avoids.
    const addListener = vi.mocked(browser.storage.onChanged.addListener);
    addListener.mockClear();

    await bootstrapNotice();

    expect(addListener).toHaveBeenCalled();
  });

  it('leaves the notice line hidden rather than throwing when no tab id can be read', async () => {
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);

    await expect(bootstrapNotice()).resolves.toBeUndefined();
    expect(hidden('answers-notice')).toBe(true);
  });
});

// ── openAnswerPanel (#btn-open-panel, ADR-044 decision 10a) ─────────────────
// Neither `sidePanel` nor `sidebarAction` is on the shared browser mock (most
// tests need neither), so each test here adds only the ONE the browser under
// test would expose, and removes it afterwards — proving the click handler
// picks the right API rather than assuming Chrome.

describe('openAnswerPanel (#btn-open-panel)', () => {
  type MutableBrowser = typeof browser & {
    sidePanel?: { open: (o: { tabId: number }) => Promise<void> };
    sidebarAction?: { open: () => Promise<void> };
  };
  const mutableBrowser = browser as MutableBrowser;

  afterEach(() => {
    delete mutableBrowser.sidePanel;
    delete mutableBrowser.sidebarAction;
  });

  it('calls chrome.sidePanel.open with the active tab id, synchronously from the click', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidePanel = { open };
    await bootstrapNotice(); // (re)resolves activeTabId from the tabs.query mock (id 7)

    click('btn-open-panel');

    expect(open).toHaveBeenCalledWith({ tabId: 7 });
  });

  it('renders the launcher label (PR0 §2)', () => {
    expect(textOf('btn-open-panel')?.trim()).toBe('Open the panel →');
  });

  it('falls back to browser.sidebarAction.open() when there is no sidePanel API (Firefox)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidebarAction = { open };
    await bootstrapNotice();

    click('btn-open-panel');

    expect(open).toHaveBeenCalledTimes(1);
  });

  it('surfaces a message rather than throwing when neither API is available', async () => {
    await bootstrapNotice();
    byId<HTMLElement>('import-msg').textContent = '';

    click('btn-open-panel');

    expect(textOf('import-msg')).toContain('no side panel');
  });

  it('reports the unresolved tab, not a false "no side panel", when sidePanel exists but activeTabId has not resolved yet (regression)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidePanel = { open };
    // No tab from `tabs.query` this time — `activeTabId` stays `null`.
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);
    await bootstrapNotice();
    byId<HTMLElement>('import-msg').textContent = '';

    click('btn-open-panel');

    // Chrome DOES have a side panel here — the true cause is the unresolved
    // tab id, and the message must say so instead of the Firefox-shaped "this
    // browser has no side panel" line, which is false on this browser.
    expect(open).not.toHaveBeenCalled();
    expect(textOf('import-msg')).not.toContain('no side panel');
    expect(textOf('import-msg')).toContain('this tab');
  });
});

// ── the one-shot save-answers-on-submit auto-save notice (PR4, decision 7) ──

describe('checkAutoSaveNotice', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLElement>('auto-save-notice').hidden = true;
    byId<HTMLElement>('auto-save-notice-text').textContent = '';
  });

  it('shows the notice text and reveals the banner when one is pending', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'autoSaveNotice',
      text: 'Saved 1 answer from this submit — change this in Settings.',
    });

    await checkAutoSaveNotice();

    expect(hidden('auto-save-notice')).toBe(false);
    expect(textOf('auto-save-notice-text')).toBe(
      'Saved 1 answer from this submit — change this in Settings.'
    );
  });

  it('leaves the banner hidden when nothing is pending', async () => {
    sendMessageMock.mockResolvedValueOnce({ ok: true, kind: 'autoSaveNotice', text: null });

    await checkAutoSaveNotice();

    expect(hidden('auto-save-notice')).toBe(true);
  });

  it('leaves the banner hidden rather than throwing on a transport rejection', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    await expect(checkAutoSaveNotice()).resolves.toBeUndefined();
    expect(hidden('auto-save-notice')).toBe(true);
  });

  it('the dismiss button hides the banner', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'autoSaveNotice',
      text: 'Saved 1 answer from this submit.',
    });
    await checkAutoSaveNotice();
    expect(hidden('auto-save-notice')).toBe(false);

    click('auto-save-notice-dismiss');
    await flush();

    expect(hidden('auto-save-notice')).toBe(true);
  });
});

// ── theme live-sync (#1236 Half A) ───────────────────────────────────────────
// popup.ts calls `subscribeThemeChanges()` at module load (right after
// `bootTheme()`), so a `storage.local` change to `theme` — e.g. from the
// options page — repaints the ALREADY-OPEN popup instead of waiting for the
// next open. Registered listeners (theme + answer-state's session-only one)
// are fired at; only the theme listener is allowed to repaint.

describe('theme live-sync (#1236 Half A)', () => {
  it('repaints the open popup on a local theme change', async () => {
    delete document.documentElement.dataset.theme;
    // popup.ts only runs its module-load wiring once, at file scope — and
    // Vitest 5's clearMocks (default ON in this file) wipes that history
    // before each test. Re-import fresh so `subscribeThemeChanges()` registers
    // its listener during THIS test, captured below.
    vi.resetModules();
    await import('../popup');
    await flush();

    expect(document.documentElement.dataset.theme).toBeUndefined();
    const listeners = vi
      .mocked(browser.storage.onChanged.addListener)
      .mock.calls.map((call) => call[0] as (changes: unknown, areaName: string) => void);
    expect(listeners.length).toBeGreaterThan(0);
    for (const listener of listeners) listener({ theme: { newValue: 'dark' } }, 'local');

    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});
