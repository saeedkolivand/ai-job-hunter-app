/**
 * Resolution check for the PR F location keys. Uses the REAL
 * @ajh/translations instance (not the identity mock the component/chip tests
 * use) so this verifies each key resolves and the pluralized chip
 * interpolates `{{count}}` correctly per locale.
 *
 * NOTE: because @ajh/translations initializes with `fallbackLng: 'en'`, a key
 * missing in de would still resolve here (to the English string), so this
 * file does NOT catch locale gaps — that's owned by the global
 * `i18n/translations-parity.test.ts`, which reads the raw resource trees
 * directly per-locale via `i18n.getResourceBundle` + `flatten`.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('PR F location i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the picker hint label', (lng) => {
    const key = 'jobs.locationFilterHint';
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it.each([
    ['en', 1],
    ['en', 7],
    ['de', 1],
    ['de', 7],
  ] as const)('%s resolves the pluralized location-filtered chip (count=%i)', (lng, count) => {
    const key = 'jobs.boardSummary.note.locationFiltered';
    expect(i18n.exists(key, { lng, count, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key, { count });
    // Resolved (not the raw key) and the count was interpolated.
    expect(out).not.toBe(key);
    expect(out).toContain(String(count));
  });

  it.each(LOCALES)('%s resolves the n=0 plain marker label', (lng) => {
    const key = 'jobs.boardSummary.note.locationFilteredNone';
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });
});
