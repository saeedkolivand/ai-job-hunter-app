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
 *
 * Robustness notes:
 *  - All DOM queries are scoped to the `container` returned by `render()` to
 *    prevent interaction with parallel tests that share the global `document`.
 *  - Raw `getAttribute('style')` is used instead of `.style.background` everywhere
 *    so that jsdom CSSOM re-serialization (which can normalise hex → rgb or drop
 *    the value depending on parse order) never affects our assertions.
 *  - `getSwatchButtons` filters to buttons whose raw style attribute contains BOTH
 *    `linear-gradient(` AND a `#` hex stop, which excludes the Default chip's
 *    `<span>` (CSS-var gradient, no `#`) and any other non-hex-gradient elements.
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
 * All preset accent swatch buttons within `root`: the ones rendered by the
 * ACCENTS array in AppearanceCard. Identified by their raw inline style
 * attribute containing BOTH `linear-gradient(` AND a `#` hex stop — this
 * reliably excludes the Default chip's `<span>` (which uses CSS vars, no `#`)
 * and any buttons without a gradient style at all.
 *
 * Using `getAttribute('style')` instead of `.style.background` avoids jsdom
 * CSSOM re-serialization, which can normalize hex stops to `rgb(...)` or drop
 * the value entirely depending on parse/run order across the full test suite.
 */
function getSwatchButtons(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('button')).filter((btn) => {
    const rawStyle = btn.getAttribute('style') ?? '';
    return rawStyle.includes('linear-gradient(') && rawStyle.includes('#');
  });
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AppearanceCard — preset accent swatches', () => {
  it('renders 8 preset swatch buttons (one per ACCENTS entry)', () => {
    const { container } = render(<AppearanceCard />);
    // ACCENTS has 8 entries (violet, blue, green, orange, pink, red, yellow, graphite).
    expect(getSwatchButtons(container)).toHaveLength(8);
  });

  it('every preset swatch carries a linear-gradient background, not a flat color', () => {
    const { container } = render(<AppearanceCard />);
    const swatches = getSwatchButtons(container);
    expect(swatches.length).toBeGreaterThan(0);
    for (const btn of swatches) {
      expect(btn.getAttribute('style') ?? '').toContain('linear-gradient(');
    }
  });

  it('the Default chip colour dot uses the CSS-var gradient (var(--color-brand) … var(--color-brand-2))', () => {
    const { container } = render(<AppearanceCard />);
    // The Default chip contains a <span> whose background is the CSS-var gradient.
    const dots = Array.from(container.querySelectorAll<HTMLElement>('span')).filter((el) =>
      (el.getAttribute('style') ?? '').includes('var(--color-brand)')
    );
    expect(dots.length).toBeGreaterThanOrEqual(1);
    const dot = dots[0];
    if (!dot) throw new Error('expected default chip dot with CSS-var gradient background');
    const dotStyle = dot.getAttribute('style') ?? '';
    expect(dotStyle).toContain('var(--color-brand)');
    expect(dotStyle).toContain('var(--color-brand-2)');
    expect(dotStyle).toContain('linear-gradient(');
  });

  it('clicking a preset swatch calls applyThemeAnimated with both accentColor and accentColor2', async () => {
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AppearanceCard />);

    const swatches = getSwatchButtons(container);
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
    const { container } = render(<AppearanceCard />);
    const swatches = getSwatchButtons(container);
    for (const btn of swatches) {
      // Read the raw style attribute — not the CSSOM — to avoid jsdom re-serialization
      // converting hex stops to rgb() or dropping the value under parallel test runs.
      const rawStyle = btn.getAttribute('style') ?? '';
      // Extract the two hex values from `linear-gradient(135deg, #xxxxxx, #yyyyyy)`.
      const matches = rawStyle.match(/#[0-9a-fA-F]{6}/g);
      expect(matches).not.toBeNull();
      if (!matches) throw new Error(`Expected hex matches in gradient: ${rawStyle}`);
      expect(matches.length).toBe(2);
      // The two stops must differ — the gradient is genuinely two-tone.
      const stop1 = matches[0];
      const stop2 = matches[1];
      if (!stop1 || !stop2) throw new Error(`Expected two hex stops in gradient: ${rawStyle}`);
      expect(stop1.toLowerCase()).not.toBe(stop2.toLowerCase());
    }
  });
});
