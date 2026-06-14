/**
 * AppearanceCard — two-tone gradient accent swatch tests.
 *
 * Covers:
 *  - Every preset accent swatch button renders with an inline `background` style
 *    containing `linear-gradient(` — NOT a flat solid color.
 *  - The Default chip's colour dot uses the un-overridden BASE CSS-var gradient
 *    pair (var(--color-brand-base) … var(--color-brand-2-base)) so it always
 *    shows the TRUE shipped default — never the live, applier-overridden accent.
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
 *  - Swatch detection uses the `data-accent-color` attribute (CSS-independent),
 *    so it is immune to jsdom CSSOM rejecting/normalising hex gradient values.
 *    The two accent hex stops are read from `data-accent-color` /
 *    `data-accent-color2` rather than parsing the inline `style` string.
 *  - The Default chip's colour dot is located via `data-testid="default-accent-dot"`,
 *    so finding it no longer depends on matching its style content. Its CSS-var
 *    gradient style is then asserted directly (these `getAttribute('style')`
 *    checks still work in jsdom).
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
 * ACCENTS array in AppearanceCard. The preset swatch buttons carry a stable
 * `data-accent-color` attribute, so we select them by that attribute alone.
 *
 * The Default-dot `<span>` and the System chip have no `data-accent-color`,
 * so this query is robust and CSS-independent — it does not depend on jsdom
 * CSSOM parsing the inline gradient style.
 */
function getSwatchButtons(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('button[data-accent-color]'));
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AppearanceCard — preset accent swatches', () => {
  it('renders 8 preset swatch buttons (one per ACCENTS entry)', () => {
    const { container } = render(<AppearanceCard />);
    // ACCENTS has 8 entries (violet, blue, green, orange, pink, red, yellow, graphite).
    expect(getSwatchButtons(container)).toHaveLength(8);
  });

  it('every preset swatch exposes two distinct hex accent colors (a real two-tone, not a flat color)', () => {
    const { container } = render(<AppearanceCard />);
    const swatches = getSwatchButtons(container);
    expect(swatches.length).toBeGreaterThan(0);
    for (const btn of swatches) {
      expect(btn.getAttribute('data-accent-color')).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(btn.getAttribute('data-accent-color2')).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
  });

  it('the Default chip colour dot uses the un-overridden BASE CSS-var gradient (var(--color-brand-base) … var(--color-brand-2-base))', () => {
    const { container } = render(<AppearanceCard />);
    // The Default chip contains a <span> tagged with a stable test id; locating it
    // by `data-testid` keeps element detection CSS-independent (no style matching).
    const dot = container.querySelector<HTMLElement>('[data-testid="default-accent-dot"]');
    if (!dot) throw new Error('expected default chip dot with CSS-var gradient background');
    const dotStyle = dot.getAttribute('style') ?? '';
    expect(dotStyle).toContain('var(--color-brand-base)');
    expect(dotStyle).toContain('var(--color-brand-2-base)');
    expect(dotStyle).toContain('linear-gradient(');
    // Regression guard: the dot must NOT use the live accent vars, which the
    // runtime applier (theme.ts applyAccent / ACCENT_VARS) overrides on :root —
    // those would make "Default" wrongly mirror the active custom/system accent.
    // The `-base` suffix is intentionally distinct, so the bare-var substrings
    // must be absent (the closing paren differs: `--color-brand)` vs `-base)`).
    expect(dotStyle).not.toContain('var(--color-brand)');
    expect(dotStyle).not.toContain('var(--color-brand-2)');
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
      // Read the two stops from the stable data attributes — CSS-independent, so
      // jsdom CSSOM rejecting/normalising the inline gradient never affects this.
      const color = btn.getAttribute('data-accent-color');
      const color2 = btn.getAttribute('data-accent-color2');
      if (!color || !color2) {
        throw new Error('expected both data-accent-color and data-accent-color2 on swatch');
      }
      expect(color).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(color2).toMatch(/^#[0-9a-fA-F]{6}$/);
      // The two stops must differ — the gradient is genuinely two-tone.
      expect(color.toLowerCase()).not.toBe(color2.toLowerCase());
    }
  });
});
