import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  applyTheme,
  getResolvedScheme,
  getThemePrefs,
  restoreTheme,
  type ThemePrefs,
} from './theme';

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
    document.documentElement.removeAttribute('data-text-scale');
    document.documentElement.className = '';
    document.documentElement.style.cssText = '';
    stubMatchMedia({});
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it('applies an explicit scheme to data-color-scheme + class + storage', () => {
    applyTheme({
      scheme: 'light',
      reduceTransparency: false,
      contrast: 'normal',
      textScale: 'default',
      accentSource: 'default',
    });
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
    applyTheme({
      scheme: 'dark',
      reduceTransparency: true,
      contrast: 'more',
      textScale: 'default',
      accentSource: 'default',
    });
    expect(document.documentElement.hasAttribute('data-reduce-transparency')).toBe(true);
    expect(document.documentElement.dataset.contrast).toBe('more');
  });

  it('auto-detects reduce-transparency from the OS preference', () => {
    stubMatchMedia({ '(prefers-reduced-transparency: reduce)': true });
    applyTheme({
      scheme: 'dark',
      reduceTransparency: false,
      contrast: 'normal',
      textScale: 'default',
      accentSource: 'default',
    });
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
    applyTheme({
      scheme: 'light',
      reduceTransparency: false,
      contrast: 'normal',
      textScale: 'default',
      accentSource: 'default',
    });
    document.documentElement.removeAttribute('data-color-scheme');
    restoreTheme();
    expect(document.documentElement.dataset.colorScheme).toBe('light');
  });

  it('defaults to the system scheme when nothing is persisted', () => {
    expect(getThemePrefs().scheme).toBe('system');
  });

  it('applies the text scale and defaults to "default"', () => {
    expect(getThemePrefs().textScale).toBe('default');
    applyTheme({
      scheme: 'dark',
      reduceTransparency: false,
      contrast: 'normal',
      textScale: 'large',
      accentSource: 'default',
    });
    expect(document.documentElement.dataset.textScale).toBe('large');
    expect(JSON.parse(localStorage.getItem('ajh-theme') ?? '{}').textScale).toBe('large');
  });
});

describe('theme engine — accent', () => {
  const base: ThemePrefs = {
    scheme: 'dark',
    reduceTransparency: false,
    contrast: 'normal',
    textScale: 'default',
    accentSource: 'default',
  };
  const cssVar = (name: string) => document.documentElement.style.getPropertyValue(name);

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.style.cssText = '';
    stubMatchMedia({});
  });
  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it("'default' source writes no inline accent override", () => {
    applyTheme({ ...base, accentSource: 'default', accentColor: '#a855f7' });
    expect(cssVar('--color-brand')).toBe('');
    expect(cssVar('--color-brand-soft')).toBe('');
    expect(cssVar('--color-action-foreground')).toBe('');
  });

  it("'custom' source sets brand + a lighter soft step + an auto-contrast label", () => {
    applyTheme({ ...base, accentSource: 'custom', accentColor: '#a855f7' });
    expect(cssVar('--color-brand')).toBe('#a855f7');
    // soft is derived (lighter) and a valid hex, not equal to the base accent.
    const soft = cssVar('--color-brand-soft');
    expect(soft).toMatch(/^#[0-9a-f]{6}$/);
    expect(soft).not.toBe('#a855f7');
    // a dark violet → near-white label.
    expect(cssVar('--color-action-foreground')).toBe('#ffffff');
  });

  it('gives a pale accent a dark label (auto-contrast)', () => {
    applyTheme({ ...base, accentSource: 'custom', accentColor: '#ffe680' });
    expect(cssVar('--color-action-foreground')).toBe('#1d1d1f');
  });

  it('falls back to default (clears overrides) on an invalid color', () => {
    applyTheme({ ...base, accentSource: 'custom', accentColor: 'not-a-color' });
    expect(cssVar('--color-brand')).toBe('');
  });

  it('clears a previously applied accent when switching back to default', () => {
    applyTheme({ ...base, accentSource: 'custom', accentColor: '#a855f7' });
    applyTheme({ ...base, accentSource: 'default' });
    expect(cssVar('--color-brand')).toBe('');
    expect(cssVar('--color-brand-soft')).toBe('');
    expect(cssVar('--color-action-foreground')).toBe('');
  });

  it("'system' source applies its resolved color like custom", () => {
    applyTheme({ ...base, accentSource: 'system', accentColor: '#22c55e' });
    expect(cssVar('--color-brand')).toBe('#22c55e');
  });

  it('lifts the soft step more on the dark canvas than the light canvas', () => {
    applyTheme({ ...base, scheme: 'dark', accentSource: 'custom', accentColor: '#7c3aed' });
    const darkSoft = cssVar('--color-brand-soft');
    applyTheme({ ...base, scheme: 'light', accentSource: 'custom', accentColor: '#7c3aed' });
    const lightSoft = cssVar('--color-brand-soft');
    expect(darkSoft).not.toBe(lightSoft);
  });
});
