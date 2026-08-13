/**
 * Guards the light-scheme status TEXT remaps in `utilities.css`.
 *
 * Why a test and not a review note: these rules are plain CSS overrides keyed on
 * `[data-color-scheme='light']`, and a SECOND rule for the same class later in
 * the file silently wins at equal specificity. That is exactly how the first
 * attempt at this fix shipped inert — the file already carried a canonical block
 * ~120 lines below the one that was added, so the new (deeper) values never
 * applied and the measured contrast never moved. Nothing in the type system,
 * the linter, or a component test can see that; only the stylesheet can.
 *
 * Parses the source stylesheet (not the built bundle) so the check runs in the
 * normal unit suite with no build step.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const HERE = dirname(fileURLToPath(import.meta.url));
const CSS = readFileSync(join(HERE, 'utilities.css'), 'utf8');
const TOKENS = readFileSync(join(HERE, 'tokens.css'), 'utf8');

/**
 * Each remapped utility, the named token it must reference, and the palette step
 * that token must resolve to. The indirection is the point: `utilities.css`
 * states intent, `tokens.css` owns the value.
 */
const LIGHT_REMAPS: ReadonlyArray<{
  cls: string;
  token: string;
  resolvesTo: string;
  why: string;
}> = [
  {
    cls: 'text-red-400',
    token: '--color-status-error-text',
    resolvesTo: 'var(--color-red-700)',
    why: 'AA on the tinted error fill',
  },
  {
    cls: 'text-amber-400',
    token: '--color-status-warning-text',
    resolvesTo: 'var(--color-amber-800)',
    why: 'AA on the tinted warning fill',
  },
  {
    cls: 'text-brand-soft',
    token: '--color-status-accent-text',
    resolvesTo: 'var(--color-brand)',
    why: 'the dark-canvas accent lift washes out on white',
  },
];

/** Every `[data-color-scheme='light'] .<cls> { … }` rule body, in file order. */
function lightRules(cls: string): string[] {
  const re = new RegExp(
    String.raw`\[data-color-scheme=['"]?light['"]?\]\s*\.` +
      cls.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') +
      String.raw`\s*\{([^}]*)\}`,
    'g'
  );
  return [...CSS.matchAll(re)].map((m) => (m[1] ?? '').replace(/\s+/g, ' ').trim());
}

/**
 * The declaration block of the first rule whose SELECTOR LIST contains
 * `selector`, or `null` when no rule does — so an assertion about a selector
 * that was renamed away fails loudly instead of quietly matching a comment.
 */
function ruleContaining(selector: string): string | null {
  for (const [, selectors = '', body = ''] of CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (selectors.includes(selector)) return body.replace(/\s+/g, ' ').trim();
  }
  return null;
}

/**
 * Every status TEXT class the light scheme remaps — including the two that
 * still carry a literal (emerald/blue), which the token-indirection assertions
 * below deliberately skip.
 *
 * They belong in the duplicate-rule check regardless: a second rule for any of
 * these silently wins at equal specificity, which is the failure mode this file
 * exists for. Callers must reach these through the BARE class — Tailwind v4
 * compiles an opacity suffix (`text-amber-300/70`, `text-amber-400/80`) into a
 * separate class over the RAW palette variable, which no remap here can touch.
 */
const REMAPPED_STATUS_TEXT = [
  'text-emerald-400',
  'text-blue-400',
  'text-amber-400',
  'text-red-400',
  'text-brand-soft',
] as const;

describe('utilities.css — light-scheme status text remaps', () => {
  it.each(REMAPPED_STATUS_TEXT)('.%s has exactly ONE light rule', (cls) => {
    expect(lightRules(cls)).toHaveLength(1);
  });

  it.each(LIGHT_REMAPS)('.$cls has exactly ONE light rule', ({ cls }) => {
    // Two rules at equal specificity ⇒ the later one silently wins and the
    // earlier one is dead. That is the bug this whole file exists to prevent.
    expect(lightRules(cls)).toHaveLength(1);
  });

  it.each(LIGHT_REMAPS)('.$cls references its named token ($why)', ({ cls, token }) => {
    expect(lightRules(cls)[0]).toContain(`var(${token})`);
  });

  it.each(LIGHT_REMAPS)('$token resolves to the deepened step', ({ token, resolvesTo }) => {
    // Defined exactly once, in the light block of tokens.css.
    const decls = [...TOKENS.matchAll(new RegExp(`${token}:\\s*([^;]+);`, 'g'))];
    expect(decls).toHaveLength(1);
    expect(decls[0]?.[1]?.trim()).toBe(resolvesTo);
  });

  it('keeps the raw colour literals out of utilities.css', () => {
    // The whole point of the token indirection: values live in tokens.css.
    for (const { cls } of LIGHT_REMAPS) {
      expect(lightRules(cls)[0]).not.toMatch(/rgb\(|#[0-9a-f]{3,8}\b|oklch\(/i);
    }
  });

  it('does not regress to the shallower -600/-700 steps the audit measured under AA', () => {
    // The pre-existing block shipped red-600 / amber-700, which measured
    // 4.02–4.36:1 on the tinted Tag fills — under the 4.5:1 floor.
    expect(TOKENS).not.toMatch(/--color-status-error-text:\s*var\(--color-red-600\)/);
    expect(TOKENS).not.toMatch(/--color-status-warning-text:\s*var\(--color-amber-700\)/);
  });

  /**
   * Not a contrast rule, but the same class of defect: a sitewide CSS selector
   * list nothing else can see.
   *
   * Selector AND declaration are asserted together. The previous version
   * checked only that a selector string occurred somewhere in the file, which a
   * rule setting the wrong cursor — or no cursor at all — passes just as
   * happily. It also "guarded" a `[role='radio']` entry that could never fire:
   * every radio in this app is a `<button>` (SegmentedControl, AppearanceCard,
   * AccentPicker, LetterLayoutPicker), so `button:not(:disabled)` had already
   * matched it, and the entry's own comment claimed the opposite.
   */
  describe('the pointer-cursor rule', () => {
    it.each([
      ["[role='button']:not([aria-disabled='true'])", 'cursor: pointer'],
      ["[role='tab']:not([aria-disabled='true'])", 'cursor: pointer'],
      // THE fix: `aria-disabled` is not `:disabled`, so `button:not(:disabled)`
      // still matches an aria-disabled button and hands it the pointer — and
      // the component's own Tailwind `cursor-not-allowed` is layered, which
      // cannot out-rank an unlayered rule at any specificity. The arrow comes
      // back from here or not at all.
      ["button[aria-disabled='true']", 'cursor: not-allowed'],
    ])('%s declares %s', (selector, declaration) => {
      expect(ruleContaining(selector)).toContain(declaration);
    });

    it('drops the inert [role=radio] entry rather than leaving it to be trusted', () => {
      expect(CSS).not.toContain("[role='radio']");
    });
  });

  it('scopes every remap to the light scheme only (dark is untouched)', () => {
    for (const { cls } of LIGHT_REMAPS) {
      // A bare `.text-red-400 { … }` override in this file would hit BOTH
      // schemes; the palette-level definition lives in tokens.css, not here.
      const bare = new RegExp(String.raw`(^|\})\s*\.` + cls + String.raw`\s*\{`, 'm');
      expect(CSS).not.toMatch(bare);
    }
  });
});

/**
 * Guards the `text-foreground\/NN` and `border-foreground\/NN` opacity-suffix
 * remaps: Tailwind compiles each opacity step (`/15`, `/55`, …) into its OWN
 * literal escaped selector over the raw `--color-foreground` token, so a step
 * outside the enumeration below renders at its raw, un-boosted alpha on
 * light — near-invisible for the low steps this file boosts (WCAG ~1.2–1.7:1
 * measured; see `docs/NEXT_ISSUES.md` "the text-foreground/NN opacity ramp").
 *
 * The contrast math is inlined (not imported) so this test independently
 * recomputes the ratio from the LITERAL `color-mix(...) NN%` in the
 * stylesheet — a red-first pin: dial a percentage back down toward its raw
 * value and the ratio assertion fails, it does not just start trusting a
 * stale constant.
 */
describe('utilities.css — text/border-foreground opacity remaps (contrast)', () => {
  // light-scheme --color-foreground is Apple ink, documented in tokens.css as
  // `#1d1d1f`. WHITE stands in for the card/canvas the text/border sits on.
  const INK_CHANNEL = 0x1d / 255;
  const WHITE_CHANNEL = 1;

  function srgbToLinear(c: number): number {
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }

  /** Relative luminance of the grayscale mix of ink and white at `alphaPct`
   *  (the same math the browser does compositing `color-mix(..., NN%, transparent)`
   *  — i.e. plain alpha-over-white on the raw sRGB channel, then linearized). */
  function luminanceAtAlpha(alphaPct: number): number {
    const a = alphaPct / 100;
    const channel = a * INK_CHANNEL + (1 - a) * WHITE_CHANNEL;
    return srgbToLinear(channel);
  }

  const WHITE_LUMINANCE = 1; // srgbToLinear(1) === 1

  function contrastAtAlpha(alphaPct: number): number {
    return (WHITE_LUMINANCE + 0.05) / (luminanceAtAlpha(alphaPct) + 0.05);
  }

  /** The `NN%` color-mix weight for the first light rule matching `cls`, or
   *  `null` if no rule (or an unrecognized declaration shape) is found. */
  function remappedAlphaPct(cls: string): number | null {
    const body = lightRules(cls)[0];
    const m = body?.match(/var\(--color-foreground\)\s*(\d+(?:\.\d+)?)%/);
    return m?.[1] ? Number(m[1]) : null;
  }

  const TEXT_STEPS = [15, 20, 25, 30, 35, 40, 45, 50, 55, 60] as const;
  const BORDER_STEPS = [10, 12, 15, 20, 25] as const;

  it.each(TEXT_STEPS)('.text-foreground\\/%i has exactly ONE light rule', (nn) => {
    expect(lightRules(`text-foreground\\/${nn}`)).toHaveLength(1);
  });

  it.each(BORDER_STEPS)('.border-foreground\\/%i has exactly ONE light rule', (nn) => {
    expect(lightRules(`border-foreground\\/${nn}`)).toHaveLength(1);
  });

  it.each([15, 55, 60] as const)(
    '.text-foreground\\/%i is boosted strictly above its raw (unmapped) alpha',
    (nn) => {
      const boosted = remappedAlphaPct(`text-foreground\\/${nn}`);
      expect(boosted).not.toBeNull();
      expect(boosted as number).toBeGreaterThan(nn);
    }
  );

  it.each(BORDER_STEPS)(
    '.border-foreground\\/%i is boosted strictly above its raw (unmapped) alpha',
    (nn) => {
      const boosted = remappedAlphaPct(`border-foreground\\/${nn}`);
      expect(boosted).not.toBeNull();
      expect(boosted as number).toBeGreaterThan(nn);
    }
  );

  it.each([55, 60] as const)(
    'text-foreground/%i raw (unmapped) alpha fails AA — the defect this remap fixes',
    (nn) => {
      // /60's raw contrast (~4.45:1) is JUST as sub-AA as /55's (~3.8:1) —
      // neither clears 4.5:1 unmapped. A prior version of this comment
      // claimed /60 "already clears 4.5" raw; it does not.
      expect(contrastAtAlpha(nn)).toBeLessThan(4.5);
    }
  );

  it.each([55, 60] as const)('text-foreground/%i clears 4.5:1 once boosted to its remapped alpha', (nn) => {
    const boosted = remappedAlphaPct(`text-foreground\\/${nn}`);
    expect(boosted).not.toBeNull();
    expect(contrastAtAlpha(boosted as number)).toBeGreaterThanOrEqual(4.5);
  });

  it('preserves ordering at the /55↔/60 seam: /60 must map to MORE alpha than /55, not less', () => {
    // Regression guard for the exact defect this file's own history caught:
    // /55 was boosted (69%) without also boosting /60, so a LOWER opacity
    // step (55) briefly rendered MORE opaque than a HIGHER one (60, still raw
    // at 60%) — the ordering a `text-foreground/NN` scale promises callers
    // inverted at that one seam. Mapping /60 too restores it.
    const alpha55 = remappedAlphaPct('text-foreground\\/55');
    const alpha60 = remappedAlphaPct('text-foreground\\/60');
    expect(alpha55).not.toBeNull();
    expect(alpha60).not.toBeNull();
    expect(alpha60 as number).toBeGreaterThan(alpha55 as number);
  });

  it('text-foreground/65 is left deliberately unmapped — it clears AA on its own raw alpha', () => {
    // The new boundary (was /60 before this fix folded /60 into the remap
    // above). /65's raw 65% alpha independently clears 4.5:1, so it needs no
    // remap of its own — extending the curve further would re-tint hundreds
    // of call sites for zero contrast gain. (Its raw 65% sits a little below
    // the now-mapped /60's 72% — a much smaller, purely cosmetic ordering
    // wrinkle between two already-AA-passing steps, not the "renders
    // near-invisible" defect this file exists to fix; left as-is.)
    expect(lightRules('text-foreground\\/65')).toHaveLength(0);
    expect(contrastAtAlpha(65)).toBeGreaterThanOrEqual(4.5);
  });

  it.each(BORDER_STEPS)(
    'border-foreground\\/%i raw (unmapped) alpha fails 3:1 — the defect this remap fixes',
    (nn) => {
      expect(contrastAtAlpha(nn)).toBeLessThan(3);
    }
  );

  it('border-foreground/70 is left unmapped because it already clears 3:1 raw', () => {
    expect(lightRules('border-foreground\\/70')).toHaveLength(0);
    expect(contrastAtAlpha(70)).toBeGreaterThanOrEqual(3);
  });
});
