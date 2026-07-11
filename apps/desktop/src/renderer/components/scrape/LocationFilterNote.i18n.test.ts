/**
 * en/de parity for the PR F location keys. Uses the REAL @ajh/translations
 * instance (not the identity mock the component/chip tests use) so a key added
 * to one locale but not the other — which resolves to the raw key string in the
 * missing locale — fails here instead of silently shipping an untranslated UI.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('PR F location i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the picker hint label', (lng) => {
    const t = i18n.getFixedT(lng);
    const out = t('jobs.locationFilterHint');
    expect(out).not.toBe('jobs.locationFilterHint');
    expect(out.trim().length).toBeGreaterThan(0);
  });

  it.each([
    ['en', 1],
    ['en', 7],
    ['de', 1],
    ['de', 7],
  ] as const)('%s resolves the pluralized location-filtered chip (count=%i)', (lng, count) => {
    const t = i18n.getFixedT(lng);
    const out = t('jobs.boardSummary.note.locationFiltered', { count });
    // Resolved (not the raw key) and the count was interpolated.
    expect(out).not.toBe('jobs.boardSummary.note.locationFiltered');
    expect(out).toContain(String(count));
  });

  it.each(LOCALES)('%s resolves the n=0 plain marker label', (lng) => {
    const t = i18n.getFixedT(lng);
    const out = t('jobs.boardSummary.note.locationFilteredNone');
    expect(out).not.toBe('jobs.boardSummary.note.locationFilteredNone');
    expect(out.trim().length).toBeGreaterThan(0);
  });
});
