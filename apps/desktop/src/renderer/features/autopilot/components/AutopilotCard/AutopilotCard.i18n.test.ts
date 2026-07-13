/**
 * Resolution check for the PR H provisional-score hint. Uses the REAL
 * @ajh/translations instance (not the identity mock AutopilotCard.test.tsx
 * uses) so this verifies the key resolves to real, non-empty copy per locale.
 *
 * NOTE: because @ajh/translations initializes with `fallbackLng: 'en'`, a key
 * missing in de would still resolve here (to the English string), so this
 * file does NOT catch locale gaps — that's owned by the global
 * `i18n/translations-parity.test.ts`, which reads the raw resource trees
 * directly per-locale via `i18n.exists(key, { lng, fallbackLng: false })`.
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
