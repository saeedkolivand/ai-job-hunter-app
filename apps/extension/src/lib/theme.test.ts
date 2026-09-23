import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    storage: {
      local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
      onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
    },
  },
}));

const { applyTheme, bootTheme, getTheme, setTheme, subscribeThemeChanges } =
  await import('./theme');

const localGet = vi.mocked(browser.storage.local.get);
const localSet = vi.mocked(browser.storage.local.set);
const localRemove = vi.mocked(browser.storage.local.remove);
const onChangedAdd = vi.mocked(browser.storage.onChanged.addListener);
const onChangedRemove = vi.mocked(browser.storage.onChanged.removeListener);

beforeEach(() => {
  localGet.mockReset().mockResolvedValue({});
  localSet.mockReset();
  localRemove.mockReset();
  onChangedAdd.mockReset();
  onChangedRemove.mockReset();
  delete document.documentElement.dataset.theme;
});

describe('getTheme', () => {
  it('defaults to system when nothing is stored', async () => {
    expect(await getTheme()).toBe('system');
  });

  it('defaults to system for a malformed stored value', async () => {
    localGet.mockResolvedValueOnce({ theme: 'sepia' });
    expect(await getTheme()).toBe('system');
  });

  it('reads a stored light/dark choice', async () => {
    localGet.mockResolvedValueOnce({ theme: 'dark' });
    expect(await getTheme()).toBe('dark');
  });
});

describe('applyTheme', () => {
  it('sets data-theme for light/dark', () => {
    applyTheme('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('removes data-theme for system (leaves the media query in control)', () => {
    document.documentElement.dataset.theme = 'dark';
    applyTheme('system');
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});

describe('setTheme', () => {
  it('persists a light/dark choice and applies it immediately', async () => {
    await setTheme('light');
    expect(localSet).toHaveBeenCalledWith({ theme: 'light' });
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('clears the stored value for system', async () => {
    await setTheme('system');
    expect(localRemove).toHaveBeenCalledWith('theme');
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});

describe('bootTheme', () => {
  it('applies the persisted theme', async () => {
    localGet.mockResolvedValueOnce({ theme: 'dark' });
    await bootTheme();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('leaves the system default standing rather than throwing on a storage failure', async () => {
    localGet.mockRejectedValueOnce(new Error('storage unavailable'));
    await expect(bootTheme()).resolves.toBeUndefined();
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});

// ── subscribeThemeChanges (#1236 Half A): repaint the open surface when the
// Settings → Appearance → Theme choice changes elsewhere ────────────────────────

describe('subscribeThemeChanges', () => {
  /** Register the listener and return it, ready to be fired. */
  function register(): (changes: unknown, areaName: string) => void {
    subscribeThemeChanges();
    const listeners = onChangedAdd.mock.calls.map((call) => call[0] as never);
    expect(listeners).toHaveLength(1);
    return listeners[0] as (changes: unknown, areaName: string) => void;
  }

  it('repaints to a new theme when the local `theme` key changes', () => {
    const listener = register();
    listener({ theme: { newValue: 'dark' } }, 'local');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('ignores changes to unrelated keys', () => {
    document.documentElement.dataset.theme = 'dark';
    const listener = register();
    listener({ defaultPanelTab: { newValue: 'answers' } }, 'local');
    // An unrelated key must not recalibrate the current theme to system.
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('ignores changes in other storage areas (e.g. the per-tab answer state)', () => {
    const listener = register();
    listener({ theme: { newValue: 'dark' } }, 'session');
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('treats a removal of the theme key as system (media query back in control)', () => {
    document.documentElement.dataset.theme = 'dark';
    const listener = register();
    listener({ theme: { newValue: undefined } }, 'local');
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('treats a malformed stored value as system', () => {
    const listener = register();
    listener({ theme: { newValue: 'sepia' } }, 'local');
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('the returned unsubscribe removes the listener on teardown', () => {
    const unsubscribe = subscribeThemeChanges();
    expect(onChangedAdd).toHaveBeenCalledTimes(1);
    unsubscribe();
    expect(onChangedRemove).toHaveBeenCalledTimes(1);
  });
});
