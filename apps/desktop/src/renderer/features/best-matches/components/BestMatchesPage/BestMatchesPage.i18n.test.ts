/**
 * Resolution check for the `bestMatches.*` + `nav.bestMatches` keys, against
 * the REAL @ajh/translations instance. Mirrors AutopilotCard.i18n.test.ts:
 * `fallbackLng: 'en'` means a key missing in `de` still resolves here (to the
 * English string), so this file does NOT catch locale gaps — that's owned by
 * the global `i18n/translations-parity.test.ts`, which reads the raw resource
 * trees directly per-locale.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import { BEST_MATCHES_SORTS } from '@/features/best-matches/lib/sort-best-matches';

const LOCALES = ['en', 'de'] as const;

const STATIC_KEYS = [
  'nav.bestMatches',
  'bestMatches.title',
  'bestMatches.subtitle',
  'bestMatches.viewAll',
  'bestMatches.errorTitle',
  'bestMatches.errorDescription',
  'bestMatches.sort.label',
  'bestMatches.truncated',
  'bestMatches.salaryCaption',
  'bestMatches.row.view',
  'bestMatches.row.save',
  'bestMatches.row.apply',
  'bestMatches.row.dismiss',
  'bestMatches.row.dismissed',
  'bestMatches.row.undo',
  'bestMatches.row.dismissFailed',
  'bestMatches.row.undoFailed',
  'bestMatches.row.discovered',
  'bestMatches.row.foundBy',
  'bestMatches.row.pausedSuffix',
  'bestMatches.empty.noRuns.title',
  'bestMatches.empty.noRuns.description',
  'bestMatches.empty.noneQualified.title',
  'bestMatches.empty.noneQualified.description',
] as const;

describe('bestMatches i18n — en/de resolve', () => {
  const cases = LOCALES.flatMap((lng) => STATIC_KEYS.map((key) => [lng, key] as const));

  it.each(cases)('%s resolves %s to real, non-empty copy', (lng, key) => {
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const out = i18n.getFixedT(lng)(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });

  // The sort options are read from BEST_MATCHES_SORTS at runtime (the same
  // list the Dropdown renders), so a fourth sort added there must arrive with
  // its own key — a hardcoded pair here would let it ship without one.
  const sortCases = LOCALES.flatMap((lng) => BEST_MATCHES_SORTS.map((s) => [lng, s] as const));

  it.each(sortCases)('%s resolves bestMatches.sort.%s', (lng, sortBy) => {
    const key = `bestMatches.sort.${sortBy}`;
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const out = i18n.getFixedT(lng)(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it.each(LOCALES)('%s: bestMatches.row.foundBy interpolates {{name}}', (lng) => {
    const out = i18n.getFixedT(lng)('bestMatches.row.foundBy', { name: 'Frontend roles Berlin' });
    expect(out).toContain('Frontend roles Berlin');
  });

  it.each(LOCALES)('%s: bestMatches.viewAll interpolates {{count}}', (lng) => {
    const out = i18n.getFixedT(lng)('bestMatches.viewAll', { count: 7 });
    expect(out).toContain('7');
  });
});
