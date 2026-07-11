/**
 * en/de parity for the #621 seeded-companies wizard/picker disclosure. Uses the
 * REAL @ajh/translations instance (not the identity mock SeededCompaniesNote.test.tsx
 * uses) so a key present in only one locale — or a broken plural form — fails
 * here instead of shipping the raw key string as UI copy. Mirrors
 * LocationFilterNote.i18n.test.ts.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('#621 seeded-companies i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the disclosure hint label', (lng) => {
    const t = i18n.getFixedT(lng);
    const out = t('autopilot.wizard.target.seededCompanies.hint');
    expect(out).not.toBe('autopilot.wizard.target.seededCompanies.hint');
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it.each([
    ['en', 1],
    ['en', 22],
    ['de', 1],
    ['de', 22],
  ] as const)('%s resolves the pluralized "+N more" suffix (count=%i)', (lng, count) => {
    const t = i18n.getFixedT(lng);
    const out = t('autopilot.wizard.target.seededCompanies.more', { count });
    expect(out).not.toBe('autopilot.wizard.target.seededCompanies.more');
    expect(out).toContain(String(count));
    expect(out).toContain('+');
  });
});
