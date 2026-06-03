import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { applyTheme, getResolvedScheme, getThemePrefs, restoreTheme } from './theme';

// jsdom has no matchMedia — stub it so OS-preference probing is deterministic.
function stubMatchMedia(matches: Record<string, boolean>) {
  vi.stubGlobal(
    'matchMedia',
    (query: string) =>
      ({
        matches: matches[query] ?? false,
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
        onchange: null,
      }) as unknown as MediaQueryList
  );
}

describe('theme engine', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-color-scheme');
    document.documentElement.removeAttribute('data-reduce-transparency');
    document.documentElement.removeAttribute('data-contrast');
    document.documentElement.className = '';
    stubMatchMedia({});
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it('applies an explicit scheme to data-color-scheme + class + storage', () => {
    applyTheme({ scheme: 'light', reduceTransparency: false, contrast: 'normal' });
    expect(document.documentElement.dataset.colorScheme).toBe('light');
    expect(document.documentElement.classList.contains('light')).toBe(true);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(JSON.parse(localStorage.getItem('ajh-theme') ?? '{}').scheme).toBe('light');
  });

  it("resolves 'system' from prefers-color-scheme", () => {
    stubMatchMedia({ '(prefers-color-scheme: dark)': true });
    expect(getResolvedScheme('system')).toBe('dark');
    stubMatchMedia({ '(prefers-color-scheme: dark)': false });
    expect(getResolvedScheme('system')).toBe('light');
  });

  it('forces reduce-transparency and high-contrast attributes when set', () => {
    applyTheme({ scheme: 'dark', reduceTransparency: true, contrast: 'more' });
    expect(document.documentElement.hasAttribute('data-reduce-transparency')).toBe(true);
    expect(document.documentElement.dataset.contrast).toBe('more');
  });

  it('auto-detects reduce-transparency from the OS preference', () => {
    stubMatchMedia({ '(prefers-reduced-transparency: reduce)': true });
    applyTheme({ scheme: 'dark', reduceTransparency: false, contrast: 'normal' });
    expect(document.documentElement.hasAttribute('data-reduce-transparency')).toBe(true);
  });

  it('migrates legacy string prefs', () => {
    localStorage.setItem('ajh-theme', 'high-contrast');
    expect(getThemePrefs()).toEqual({
      scheme: 'dark',
      reduceTransparency: false,
      contrast: 'more',
    });
    localStorage.setItem('ajh-theme', 'reduced-glass');
    expect(getThemePrefs().reduceTransparency).toBe(true);
  });

  it('restoreTheme reapplies the persisted prefs', () => {
    applyTheme({ scheme: 'light', reduceTransparency: false, contrast: 'normal' });
    document.documentElement.removeAttribute('data-color-scheme');
    restoreTheme();
    expect(document.documentElement.dataset.colorScheme).toBe('light');
  });

  it('defaults to the system scheme when nothing is persisted', () => {
    expect(getThemePrefs().scheme).toBe('system');
  });
});
