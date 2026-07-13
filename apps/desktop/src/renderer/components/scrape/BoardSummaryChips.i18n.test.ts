/**
 * Resolution check for the PR H partial-ATS note keys. Uses the REAL
 * @ajh/translations instance (not the identity mock the chip tests use) so
 * this verifies each pluralized chip resolves and interpolates `{{count}}`
 * correctly per locale.
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

describe('PR H partial-ATS note i18n — en/de parity', () => {
  it.each([
    ['en', 1],
    ['en', 3],
    ['de', 1],
    ['de', 3],
  ] as const)('%s resolves the pluralized slugs-invalid chip (count=%i)', (lng, count) => {
    const key = 'jobs.boardSummary.note.slugsInvalid';
    expect(i18n.exists(key, { lng, count, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key, { count });
    expect(out).not.toBe(key);
    expect(out).toContain(String(count));
  });

  it.each([
    ['en', 1],
    ['en', 3],
    ['de', 1],
    ['de', 3],
  ] as const)('%s resolves the pluralized rows-dropped chip (count=%i)', (lng, count) => {
    const key = 'jobs.boardSummary.note.rowsDropped';
    expect(i18n.exists(key, { lng, count, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key, { count });
    expect(out).not.toBe(key);
    expect(out).toContain(String(count));
  });
});
