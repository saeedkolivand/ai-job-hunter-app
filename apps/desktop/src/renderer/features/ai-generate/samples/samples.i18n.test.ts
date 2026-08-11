/**
 * Resolution check for TEMPLATE_CAPTIONS (R7). Uses the REAL @ajh/translations
 * instance (not an identity mock) so this verifies every caption is an i18n
 * KEY that resolves to real, non-empty copy — not raw English text baked into
 * samples.ts. A raw-string map fails here: `i18n.exists` treats the caption's
 * own text as a dotted key path and finds nothing.
 *
 * Locale-gap coverage (en vs. de key sets) is owned by the global
 * `i18n/translations-parity.test.ts`; this file only pins the samples.ts side.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import { TEMPLATE_IDS } from '@/lib/generate';

import { TEMPLATE_CAPTIONS } from './samples';

const LOCALES = ['en', 'de'] as const;

describe('TEMPLATE_CAPTIONS — i18n resolution', () => {
  it('maps every template id to a key under the aiGenerate.templateCaption namespace', () => {
    for (const id of TEMPLATE_IDS) {
      expect(TEMPLATE_CAPTIONS[id], `caption for "${id}"`).toMatch(
        /^aiGenerate\.templateCaption\.[a-z-]+$/
      );
    }
  });

  it.each(LOCALES)('%s resolves every caption key to real, non-empty copy', (lng) => {
    for (const id of TEMPLATE_IDS) {
      const key = TEMPLATE_CAPTIONS[id];
      expect(i18n.exists(key, { lng, fallbackLng: false }), `${lng}:${key}`).toBe(true);
      const out = i18n.getFixedT(lng)(key);
      expect(out, `${lng}:${key}`).not.toBe(key);
      expect(out.trim().length, `${lng}:${key} is empty`).toBeGreaterThan(0);
    }
  });

  it('resolves a couple of ids to the expected copy, in both locales', () => {
    expect(i18n.getFixedT('en')(TEMPLATE_CAPTIONS.classic)).toBe(
      'Maximum ATS safety — single column, no color. Safe for every parser.'
    );
    expect(i18n.getFixedT('en')(TEMPLATE_CAPTIONS.jake)).toBe(
      'Ultra-minimal single column with a centred name and thin ruled headings. Dense, classic, parser-safe.'
    );
    expect(i18n.getFixedT('de')(TEMPLATE_CAPTIONS.classic)).not.toBe(
      i18n.getFixedT('en')(TEMPLATE_CAPTIONS.classic)
    );
  });
});
