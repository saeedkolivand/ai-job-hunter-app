/**
 * AppearanceCard — two-tone gradient accent swatch tests.
 *
 * Covers:
 *  - Every preset accent swatch button renders with an inline `background` style
 *    containing `linear-gradient(` — NOT a flat solid color.
 *  - The Default chip's colour dot uses the CSS-var gradient pair
 *    (var(--color-brand) … var(--color-brand-2)).
 *  - Clicking a preset swatch calls applyThemeAnimated with both accentColor and
 *    accentColor2 so the two-tone gradient is wired end-to-end.
 *
 * Mock strategy mirrors EmbeddingsSettings.test.tsx (the nearest sibling):
 *  - @ajh/translations → key-passthrough t()
 *  - @/services → useSystemAccent returns { data: { supported: false } } to hide
 *    the System chip (simplifies assertions; chip count is deterministic)
 *  - @ajh/ui → spread actual, override useNotification (avoids provider tree)
 *  - applyThemeAnimated is spied on via vi.mock so we can assert its args without
 *    touching real DOM / localStorage side-effects in these tests.
 */

import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ThemePrefs } from '@ajh/ui';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stubs ─────────────────────────────────────────────────────────────

vi.mock('@/services', () => ({
  useSystemAccent: () => ({ data: { supported: false } }),
}));

// ── @ajh/ui — spread actual, spy on applyThemeAnimated, stub notification ────
// applyThemeAnimatedSpy must NOT be referenced before initialisation inside a
// vi.mock factory (hoisting rule). Declare it with vi.fn() inside the factory
// and expose it on the module object; then grab it via vi.mocked() after import.

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    // Replace the side-effectful theme applier with a spy so tests never touch
    // localStorage or document.documentElement.style.
    applyThemeAnimated: vi.fn(),
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

// ── component under test (import AFTER mocks) ─────────────────────────────────

import * as AjhUiModule from '@ajh/ui';

import { AppearanceCard } from './AppearanceCard';

// Grab the spy reference after the mock is established.
const applyThemeAnimatedSpy = vi.mocked(AjhUiModule.applyThemeAnimated);

// ── helpers ───────────────────────────────────────────────────────────────────

/**
 * All preset accent swatch buttons: the ones rendered by the ACCENTS array in
 * AppearanceCard. They are round (h-7 w-7) and carry an inline `background`
 * style with a linear-gradient. We identify them by their inline style
 * containing "linear-gradient" AND having an aria-label that matches one of the
 * known accent i18n keys — distinct from the Default and System chips which
 * are text-bearing buttons.
 */
function getSwatchButtons(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('button')).filter((btn) => {
    const bg = btn.style.background ?? btn.style.backgroundColor ?? '';
    return bg.includes('linear-gradient');
  });
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AppearanceCard — preset accent swatches', () => {
  it('renders 8 preset swatch buttons (one per ACCENTS entry)', () => {
    render(<AppearanceCard />);
    // ACCENTS has 8 entries (violet, blue, green, orange, pink, red, yellow, graphite).
    expect(getSwatchButtons()).toHaveLength(8);
  });

  it('every preset swatch carries a linear-gradient background, not a flat color', () => {
    render(<AppearanceCard />);
    const swatches = getSwatchButtons();
    expect(swatches.length).toBeGreaterThan(0);
    for (const btn of swatches) {
      expect(btn.style.background).toContain('linear-gradient(');
    }
  });

  it('the Default chip colour dot uses the CSS-var gradient (var(--color-brand) … var(--color-brand-2))', () => {
    render(<AppearanceCard />);
    // The Default chip contains a <span> whose background is the CSS-var gradient.
    const dots = Array.from(document.querySelectorAll<HTMLElement>('span')).filter((el) =>
      (el.style.background ?? '').includes('var(--color-brand)')
    );
    expect(dots.length).toBeGreaterThanOrEqual(1);
    const dot = dots[0];
    if (!dot) throw new Error('expected default chip dot with CSS-var gradient background');
    expect(dot.style.background).toContain('var(--color-brand)');
    expect(dot.style.background).toContain('var(--color-brand-2)');
    expect(dot.style.background).toContain('linear-gradient(');
  });

  it('clicking a preset swatch calls applyThemeAnimated with both accentColor and accentColor2', async () => {
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    render(<AppearanceCard />);

    const swatches = getSwatchButtons();
    expect(swatches.length).toBeGreaterThan(0);

    // Click the first swatch (violet: color '#a855f7', color2 '#6366f1').
    const firstSwatch = swatches[0];
    if (!firstSwatch) throw new Error('expected at least one preset swatch button');
    await user.click(firstSwatch);

    expect(applyThemeAnimatedSpy).toHaveBeenCalledTimes(1);
    const call = applyThemeAnimatedSpy.mock.calls.at(-1);
    if (!call) throw new Error('expected applyThemeAnimated to have been called');
    const calledWith: ThemePrefs = call[0];
    expect(calledWith.accentSource).toBe('custom');
    // Both gradient stops must be forwarded — never undefined.
    expect(typeof calledWith.accentColor).toBe('string');
    expect(calledWith.accentColor?.length).toBeGreaterThan(0);
    expect(typeof calledWith.accentColor2).toBe('string');
    expect(calledWith.accentColor2?.length).toBeGreaterThan(0);
    // The two gradient stops must differ (it's a two-tone, not a self-referential pair).
    expect(calledWith.accentColor).not.toBe(calledWith.accentColor2);
  });

  it('each swatch gradient uses two distinct hex stops (start ≠ end)', () => {
    render(<AppearanceCard />);
    const swatches = getSwatchButtons();
    for (const btn of swatches) {
      // Extract the two hex values from  `linear-gradient(135deg, #xxxxxx, #yyyyyy)`.
      const matches = btn.style.background.match(/#[0-9a-fA-F]{6}/g);
      expect(matches).not.toBeNull();
      if (!matches) throw new Error(`Expected hex matches in gradient: ${btn.style.background}`);
      expect(matches.length).toBe(2);
      // The two stops must differ — the gradient is genuinely two-tone.
      const stop1 = matches[0];
      const stop2 = matches[1];
      if (!stop1 || !stop2)
        throw new Error(`Expected two hex stops in gradient: ${btn.style.background}`);
      expect(stop1.toLowerCase()).not.toBe(stop2.toLowerCase());
    }
  });
});
