import { describe, expect, it } from 'vitest';

import { OUTPUT_LANGUAGES, safeLocale, VALID_LOCALES } from './locales';

const CJK_CODES = ['zh', 'ja', 'ko'];

describe('locales (output-language registry)', () => {
  it('lists exactly the 11 supported locales in canonical order', () => {
    expect(VALID_LOCALES).toEqual([
      'en',
      'de',
      'fr',
      'es',
      'it',
      'tr',
      'pt',
      'ru',
      'zh',
      'ja',
      'ko',
    ]);
  });

  it('keeps OUTPUT_LANGUAGES and VALID_LOCALES in sync (no drift)', () => {
    expect(OUTPUT_LANGUAGES).toHaveLength(VALID_LOCALES.length);
    for (const lang of OUTPUT_LANGUAGES) {
      expect(VALID_LOCALES).toContain(lang.code);
    }
    // Every valid locale is backed by exactly one language entry.
    expect(OUTPUT_LANGUAGES.map((l) => l.code)).toEqual(VALID_LOCALES);
  });

  describe('safeLocale', () => {
    it('passes a supported locale through unchanged', () => {
      expect(safeLocale('fr')).toBe('fr');
      expect(safeLocale('ko')).toBe('ko');
    });

    it('clamps an unknown locale to English', () => {
      expect(safeLocale('xx')).toBe('en');
    });

    it('clamps an empty string to English', () => {
      expect(safeLocale('')).toBe('en');
    });

    it('is case-sensitive: an upper-case locale clamps to English', () => {
      expect(safeLocale('EN')).toBe('en');
    });
  });

  it('flags exactly the three CJK languages', () => {
    const flagged = OUTPUT_LANGUAGES.filter((l) => l.cjk === true).map((l) => l.code);
    expect(flagged).toEqual(CJK_CODES);

    for (const lang of OUTPUT_LANGUAGES) {
      if (CJK_CODES.includes(lang.code)) {
        expect(lang.cjk).toBe(true);
      } else {
        expect(lang.cjk).toBeFalsy();
      }
    }
  });

  it('gives every language a non-empty endonym and English name', () => {
    for (const lang of OUTPUT_LANGUAGES) {
      expect(lang.endonym.length).toBeGreaterThan(0);
      expect(lang.englishName.length).toBeGreaterThan(0);
    }
  });
});
