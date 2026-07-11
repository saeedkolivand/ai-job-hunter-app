/**
 * en/de parity for the PR H partial-ATS note keys. Uses the REAL
 * @ajh/translations instance (not the identity mock the chip tests use) so a
 * key added to one locale but not the other — which resolves to the raw key
 * string in the missing locale — fails here instead of silently shipping an
 * untranslated UI. Mirrors LocationFilterNote.i18n.test.ts (PR F).
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
    const t = i18n.getFixedT(lng);
    const out = t('jobs.boardSummary.note.slugsInvalid', { count });
    expect(out).not.toBe('jobs.boardSummary.note.slugsInvalid');
    expect(out).toContain(String(count));
  });

  it.each([
    ['en', 1],
    ['en', 3],
    ['de', 1],
    ['de', 3],
  ] as const)('%s resolves the pluralized rows-dropped chip (count=%i)', (lng, count) => {
    const t = i18n.getFixedT(lng);
    const out = t('jobs.boardSummary.note.rowsDropped', { count });
    expect(out).not.toBe('jobs.boardSummary.note.rowsDropped');
    expect(out).toContain(String(count));
  });
});
