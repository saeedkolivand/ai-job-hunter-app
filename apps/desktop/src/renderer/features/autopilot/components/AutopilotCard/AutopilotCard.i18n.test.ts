/**
 * Resolution check for the PR H provisional-score hint. Uses the REAL
 * @ajh/translations instance (not the identity mock AutopilotCard.test.tsx
 * uses) so this verifies the key resolves to real, non-empty copy per locale.
 *
 * NOTE: because @ajh/translations initializes with `fallbackLng: 'en'`, a key
 * missing in de would still resolve here (to the English string), so this
 * file does NOT catch locale gaps — that's owned by the global
 * `i18n/translations-parity.test.ts`, which reads the raw resource trees
 * directly per-locale via `i18n.getResourceBundle` + `flatten`.
 * Mirrors LocationFilterNote.i18n.test.ts (PR F).
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('PR H provisional-score i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the provisional-score hint', (lng) => {
    const key = 'autopilot.provisionalScoreHint';
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });
});

/**
 * The score METRIC label (ADR-020 addendum). `autopilot.scoreLabel.${variant}`
 * is built at runtime in `AutopilotCard`'s `scoreDetail`, so TypeScript cannot
 * check it — only a real `t()` can.
 *
 * Locale GAPS are not re-tested here (see this file's header): the global
 * `i18n/translations-parity.test.ts` already fails on an en-only key. What it
 * cannot know is the semantic requirement below — that the two labels are
 * genuinely DIFFERENT strings in each locale, which is the whole point of a
 * flip. `fallbackLng: false` is still passed to `exists` so this file's own
 * per-locale claim is honest rather than fallback-shadowed.
 */
describe('autopilot score metric labels — en/de', () => {
  const VARIANTS = ['coverage', 'combined'] as const;

  it.each(LOCALES.flatMap((lng) => VARIANTS.map((variant) => [lng, variant] as const)))(
    '%s resolves autopilot.scoreLabel.%s',
    (lng, variant) => {
      const key = `autopilot.scoreLabel.${variant}`;
      expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
      const out = i18n.getFixedT(lng)(key);
      expect(out).not.toBe(key);
      // It is a PERCENTAGE metric name in both languages — a label that dropped
      // the unit would read as a bare noun next to the tier word.
      expect(out).toContain('%');
    }
  );

  it.each(LOCALES)('%s distinguishes keyword coverage from the combined match', (lng) => {
    // Identical copy would make the flip invisible to the user — and is exactly
    // what a copy/paste translation produces.
    const t = i18n.getFixedT(lng);
    expect(t('autopilot.scoreLabel.coverage')).not.toBe(t('autopilot.scoreLabel.combined'));
  });
});
