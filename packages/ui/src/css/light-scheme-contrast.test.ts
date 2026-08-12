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
   * list nothing else can see. ARIA widgets are `div`s, so they get the arrow
   * cursor unless their role is named here — and a disabled one must keep it.
   */
  it.each(['button', 'tab', 'radio'])(
    'gives [role=%s] the pointer cursor, disabled ones excepted',
    (role) => {
      expect(CSS).toContain(`[role='${role}']:not([aria-disabled='true'])`);
    }
  );

  it('scopes every remap to the light scheme only (dark is untouched)', () => {
    for (const { cls } of LIGHT_REMAPS) {
      // A bare `.text-red-400 { … }` override in this file would hit BOTH
      // schemes; the palette-level definition lives in tokens.css, not here.
      const bare = new RegExp(String.raw`(^|\})\s*\.` + cls + String.raw`\s*\{`, 'm');
      expect(CSS).not.toMatch(bare);
    }
  });
});
