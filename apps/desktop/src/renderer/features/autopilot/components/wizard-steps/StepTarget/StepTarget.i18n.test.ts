/**
 * Resolution check for the page-budget field's copy. Uses the REAL
 * @ajh/translations instance — StepTarget.test.tsx mocks `useTranslation` to an
 * identity function, so a typo'd key renders as the key there and every
 * assertion still passes. Locale gaps are owned by the global
 * `i18n/translations-parity.test.ts`. Mirrors AutopilotCard.i18n.test.ts.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;
const KEYS = ['autopilot.wizard.target.pages', 'autopilot.wizard.target.pagesHint'] as const;

describe.each(LOCALES)('autopilot page-budget i18n — %s', (lng) => {
  it.each(KEYS)('resolves %s to real copy', (key) => {
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const out = i18n.getFixedT(lng)(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it('states no per-page job count in the hint', () => {
    // Regression lock: the hint originally claimed "≈25 jobs per page", a number
    // inherited from a deleted UI-side divisor rather than from any scraper.
    // Real page sizes differ per board (LinkedIn 10, The Muse 20) and the
    // wizard's DEFAULT board — the aggregator — ignores the page budget
    // altogether, so any fixed count here is false for every board.
    expect(i18n.getFixedT(lng)('autopilot.wizard.target.pagesHint')).not.toMatch(/\d/);
  });
});
