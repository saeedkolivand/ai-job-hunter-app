import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { applyTheme, getResolvedScheme, getThemePrefs, restoreTheme } from './theme';
import { basePrefs, resetThemeEnv, restoreThemeEnv, stubMatchMedia } from './theme/test-support';

describe('theme engine', () => {
  beforeEach(resetThemeEnv);
  afterEach(restoreThemeEnv);

  it('applies an explicit scheme to data-color-scheme + class + storage', () => {
    applyTheme({ ...basePrefs, scheme: 'light' });
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
    applyTheme({ ...basePrefs, reduceTransparency: true, contrast: 'more' });
    expect(document.documentElement.hasAttribute('data-reduce-transparency')).toBe(true);
    expect(document.documentElement.dataset.contrast).toBe('more');
  });

  it('auto-detects reduce-transparency from the OS preference', () => {
    stubMatchMedia({ '(prefers-reduced-transparency: reduce)': true });
    applyTheme({ ...basePrefs });
    expect(document.documentElement.hasAttribute('data-reduce-transparency')).toBe(true);
  });

  it('migrates legacy string prefs', () => {
    localStorage.setItem('ajh-theme', 'high-contrast');
    expect(getThemePrefs()).toEqual({
      scheme: 'dark',
      reduceTransparency: false,
      contrast: 'more',
      textScale: 'default',
      accentSource: 'default',
    });
    localStorage.setItem('ajh-theme', 'reduced-glass');
    expect(getThemePrefs().reduceTransparency).toBe(true);
  });

  it('restoreTheme reapplies the persisted prefs', () => {
    applyTheme({ ...basePrefs, scheme: 'light' });
    document.documentElement.removeAttribute('data-color-scheme');
    restoreTheme();
    expect(document.documentElement.dataset.colorScheme).toBe('light');
  });

  it('defaults to the system scheme when nothing is persisted', () => {
    expect(getThemePrefs().scheme).toBe('system');
  });

  it('applies the text scale and defaults to "default"', () => {
    expect(getThemePrefs().textScale).toBe('default');
    applyTheme({ ...basePrefs, textScale: 'large' });
    expect(document.documentElement.dataset.textScale).toBe('large');
    expect(JSON.parse(localStorage.getItem('ajh-theme') ?? '{}').textScale).toBe('large');
  });
});
