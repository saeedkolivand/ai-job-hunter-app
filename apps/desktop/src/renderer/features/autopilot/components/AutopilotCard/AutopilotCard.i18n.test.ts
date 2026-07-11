/**
 * en/de parity for the PR H provisional-score hint. Uses the REAL
 * @ajh/translations instance (not the identity mock AutopilotCard.test.tsx
 * uses) so a key present in only one locale fails here instead of shipping the
 * raw key string as UI copy. Mirrors LocationFilterNote.i18n.test.ts (PR F).
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

describe('PR H provisional-score i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves the provisional-score hint', (lng) => {
    const t = i18n.getFixedT(lng);
    const out = t('autopilot.provisionalScoreHint');
    expect(out).not.toBe('autopilot.provisionalScoreHint');
    expect(out.trim().length).toBeGreaterThan(0);
  });
});
