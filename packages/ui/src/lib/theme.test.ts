import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  applyTheme,
  applyThemeAnimated,
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

// ---------------------------------------------------------------------------
// Gradient — two-tone accent vars (--color-brand-2 / --color-brand-2-soft)
// ---------------------------------------------------------------------------

describe('theme engine — accent gradient (brand-2)', () => {
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

  it('uses the hand-tuned accentColor2 verbatim when provided (custom with color2)', () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#007aff',
      accentColor2: '#22d3ee',
    });
    expect(cssVar('--color-brand')).toBe('#007aff');
    expect(cssVar('--color-brand-2')).toBe('#22d3ee');
  });

  it('auto-rotates to a distinct brand-2 when accentColor2 is absent (custom without color2)', () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#007aff',
      // accentColor2 intentionally omitted
    });
    const brand2 = cssVar('--color-brand-2');
    // Must be set, valid hex, and different from the primary.
    expect(brand2).toMatch(/^#[0-9a-f]{6}$/);
    expect(brand2).not.toBe('');
    expect(brand2).not.toBe('#007aff');
  });

  it("'default' source clears --color-brand-2 and --color-brand-2-soft", () => {
    // First set a custom accent with brand-2.
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#a855f7',
      accentColor2: '#6366f1',
    });
    expect(cssVar('--color-brand-2')).toBe('#6366f1');

    // Switch to default — both gradient vars must be removed.
    applyTheme({ ...base, accentSource: 'default' });
    expect(cssVar('--color-brand-2')).toBe('');
    expect(cssVar('--color-brand-2-soft')).toBe('');
  });

  it('sets --color-brand-2-soft to a non-empty valid hex whenever brand-2 is set', () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#34c759',
      accentColor2: '#06b6a4',
    });
    const brand2Soft = cssVar('--color-brand-2-soft');
    expect(brand2Soft).toMatch(/^#[0-9a-f]{6}$/);
    expect(brand2Soft).not.toBe('');
  });

  it('--color-brand-2-soft is also set when brand-2 is auto-derived (no color2 supplied)', () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#ff9500',
      // no accentColor2
    });
    const brand2Soft = cssVar('--color-brand-2-soft');
    expect(brand2Soft).toMatch(/^#[0-9a-f]{6}$/);
    expect(brand2Soft).not.toBe('');
  });

  it("'system' source derives brand-2 by rotation, ignoring a stale accentColor2", () => {
    applyTheme({
      ...base,
      accentSource: 'system',
      accentColor: '#1e9e5a',
      // Leftover from a prior preset — must NOT be reused for the system accent.
      accentColor2: '#6366f1',
    });
    const brand2 = cssVar('--color-brand-2');
    // Derived via rotation of the system color, not the stale persisted pair.
    expect(brand2).toMatch(/^#[0-9a-f]{6}$/);
    expect(brand2).not.toBe('#6366f1');
  });

  it("'custom' source still honors the hand-tuned accentColor2 verbatim", () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#1e9e5a',
      accentColor2: '#22d3ee',
    });
    expect(cssVar('--color-brand-2')).toBe('#22d3ee');
  });
});

// ---------------------------------------------------------------------------
// Sweep middle — --color-brand-mid (start↔end midpoint, never stuck on gold)
// ---------------------------------------------------------------------------

describe('theme engine — accent sweep middle (brand-mid)', () => {
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

  it('sets --color-brand-mid to the start↔end MIDPOINT for a two-color custom accent', () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#000000',
      accentColor2: '#ffffff',
    });
    // Even channel midpoint of black↔white = #808080 — no shipped gold injected.
    expect(cssVar('--color-brand-mid')).toBe('#808080');
    expect(cssVar('--color-brand-mid-soft')).toMatch(/^#[0-9a-f]{6}$/);
  });

  it('sets a color-derived --color-brand-mid (not the default mid) when only one hue is given', () => {
    applyTheme({ ...base, accentSource: 'system', accentColor: '#22c55e' });
    const mid = cssVar('--color-brand-mid');
    expect(mid).toMatch(/^#[0-9a-f]{6}$/);
    // Must NOT be left as the shipped default mid (peach).
    expect(mid.toLowerCase()).not.toBe('#fad8a8');
  });

  it("'default' source removes --color-brand-mid + --color-brand-mid-soft (restores tokens.css default mid)", () => {
    applyTheme({
      ...base,
      accentSource: 'custom',
      accentColor: '#007aff',
      accentColor2: '#22d3ee',
    });
    expect(cssVar('--color-brand-mid')).not.toBe('');
    applyTheme({ ...base, accentSource: 'default' });
    expect(cssVar('--color-brand-mid')).toBe('');
    expect(cssVar('--color-brand-mid-soft')).toBe('');
  });
});

// ---------------------------------------------------------------------------
// applyThemeAnimated — View Transition gating (Linux / reduced-motion / missing API)
// ---------------------------------------------------------------------------

describe('applyThemeAnimated — view-transition gating', () => {
  const prefs: ThemePrefs = {
    scheme: 'dark',
    reduceTransparency: false,
    contrast: 'normal',
    textScale: 'default',
    accentSource: 'default',
  };

  function stubMatchMediaNoMotion() {
    vi.stubGlobal(
      'matchMedia',
      (query: string) =>
        ({
          matches: false,
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

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-color-scheme');
    document.documentElement.removeAttribute('data-reduce-transparency');
    document.documentElement.removeAttribute('data-contrast');
    document.documentElement.removeAttribute('data-text-scale');
    document.documentElement.className = '';
    document.documentElement.style.cssText = '';
    stubMatchMediaNoMotion();
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it('calls startViewTransition on non-Linux UA when the API is present', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' });
    const spy = vi.fn((cb: () => void) => {
      cb();
    });
    Object.defineProperty(document, 'startViewTransition', {
      value: spy,
      configurable: true,
      writable: true,
    });

    applyThemeAnimated(prefs);

    expect(spy).toHaveBeenCalledOnce();
    // Theme must still apply (data-color-scheme written inside the callback).
    expect(document.documentElement.dataset.colorScheme).toBe('dark');

    // Cleanup
    Object.defineProperty(document, 'startViewTransition', {
      value: undefined,
      configurable: true,
      writable: true,
    });
  });

  it('does NOT call startViewTransition on Linux UA — applies theme directly', () => {
    vi.stubGlobal('navigator', {
      userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15',
    });
    const spy = vi.fn();
    Object.defineProperty(document, 'startViewTransition', {
      value: spy,
      configurable: true,
      writable: true,
    });

    applyThemeAnimated(prefs);

    expect(spy).not.toHaveBeenCalled();
    // Theme still applied directly.
    expect(document.documentElement.dataset.colorScheme).toBe('dark');

    Object.defineProperty(document, 'startViewTransition', {
      value: undefined,
      configurable: true,
      writable: true,
    });
  });

  it('applies theme directly when startViewTransition is absent (older browsers)', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' });
    // Ensure startViewTransition is not present.
    Object.defineProperty(document, 'startViewTransition', {
      value: undefined,
      configurable: true,
      writable: true,
    });

    applyThemeAnimated(prefs);

    expect(document.documentElement.dataset.colorScheme).toBe('dark');
  });

  it('skips startViewTransition when prefers-reduced-motion is set', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' });
    vi.stubGlobal(
      'matchMedia',
      (query: string) =>
        ({
          matches: query === '(prefers-reduced-motion: reduce)',
          media: query,
          addEventListener: () => {},
          removeEventListener: () => {},
          addListener: () => {},
          removeListener: () => {},
          dispatchEvent: () => false,
          onchange: null,
        }) as unknown as MediaQueryList
    );
    const spy = vi.fn();
    Object.defineProperty(document, 'startViewTransition', {
      value: spy,
      configurable: true,
      writable: true,
    });

    applyThemeAnimated(prefs);

    expect(spy).not.toHaveBeenCalled();
    expect(document.documentElement.dataset.colorScheme).toBe('dark');

    Object.defineProperty(document, 'startViewTransition', {
      value: undefined,
      configurable: true,
      writable: true,
    });
  });
});
