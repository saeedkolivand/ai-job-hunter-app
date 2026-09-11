import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    storage: {
      local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
    },
  },
}));

const { applyTheme, bootTheme, getTheme, setTheme } = await import('./theme');

const localGet = vi.mocked(browser.storage.local.get);
const localSet = vi.mocked(browser.storage.local.set);
const localRemove = vi.mocked(browser.storage.local.remove);

beforeEach(() => {
  localGet.mockReset().mockResolvedValue({});
  localSet.mockReset();
  localRemove.mockReset();
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
