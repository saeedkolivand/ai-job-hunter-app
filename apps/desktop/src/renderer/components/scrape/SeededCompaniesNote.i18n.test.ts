/**
 * Resolution check for the #621 seeded-companies wizard/picker disclosure.
 * Uses the REAL @ajh/translations instance (not the identity mock
 * SeededCompaniesNote.test.tsx uses) so this verifies each key resolves to
 * real, non-empty copy and the pluralized "+N more" suffix interpolates
 * correctly per locale.
 *
 * NOTE: because @ajh/translations initializes with `fallbackLng: 'en'`, a key
 * missing in de would still resolve here (to the English string), so this
 * file does NOT catch locale gaps — that's owned by the global
 * `i18n/translations-parity.test.ts`, which reads the raw resource trees
 * directly per-locale via `i18n.getResourceBundle` + `flatten`.
 * Mirrors LocationFilterNote.i18n.test.ts.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('#621 seeded-companies i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the disclosure hint label', (lng) => {
    const key = 'autopilot.wizard.target.seededCompanies.hint';
    expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key);
    expect(out).not.toBe(key);
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it.each([
    ['en', 1],
    ['en', 22],
    ['de', 1],
    ['de', 22],
  ] as const)('%s resolves the pluralized "+N more" suffix (count=%i)', (lng, count) => {
    const key = 'autopilot.wizard.target.seededCompanies.more';
    expect(i18n.exists(key, { lng, count, fallbackLng: false }), `${lng}:${key}`).toBe(true);
    const t = i18n.getFixedT(lng);
    const out = t(key, { count });
    expect(out).not.toBe(key);
    expect(out).toContain(String(count));
    expect(out).toContain('+');
  });
});
