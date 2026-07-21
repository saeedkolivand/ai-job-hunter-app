import { describe, expect, it } from 'vitest';

import {
  detectLanguage,
  detectLanguages,
  getLanguageName,
  isCjkLanguage,
} from './language-detection';

// Reasonably long samples — franc needs ~enough text to be confident.
const ENGLISH =
  'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';
const GERMAN =
  'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';
const FRENCH =
  "Ingénieur logiciel expérimenté possédant une solide expérience dans la création d'applications web évolutives et de systèmes backend distribués pour de grandes organisations.";
// franc emits the INDIVIDUAL ISO 639-3 code for these — cmn / arb / nob — never
// the macrolanguage zho / ara / nor, so mapping on the macrolanguage code left
// the entry dead and every one of these resolved to 'unknown'.
const CHINESE =
  '我们正在寻找一位经验丰富的软件工程师加入我们的团队。您将负责设计和开发高质量的后端服务，与产品经理和设计师紧密合作，推动项目按时交付。';
const ARABIC =
  'نبحث عن مهندس برمجيات ذي خبرة للانضمام إلى فريقنا. ستكون مسؤولاً عن تصميم وتطوير خدمات خلفية عالية الجودة والعمل بشكل وثيق مع مديري المنتجات والمصممين لتسليم المشاريع في الوقت المحدد.';
const NORWEGIAN =
  'Vi ser etter en erfaren programvareutvikler som vil bli med i teamet vårt. Du vil ha ansvaret for å designe og utvikle backend-tjenester av høy kvalitet, og samarbeide tett med produktsjefer og designere.';

describe('detectLanguage', () => {
  it('returns "unknown" for empty or very short text', () => {
    expect(detectLanguage('')).toBe('unknown');
    expect(detectLanguage('hello')).toBe('unknown');
    expect(detectLanguage('a'.repeat(19))).toBe('unknown');
  });

  it('detects English and maps to ISO 639-1', () => {
    expect(detectLanguage(ENGLISH)).toBe('en');
  });

  it('detects German', () => {
    expect(detectLanguage(GERMAN)).toBe('de');
  });

  it('detects French', () => {
    expect(detectLanguage(FRENCH)).toBe('fr');
  });

  it('detects Chinese, Arabic and Norwegian (franc emits cmn/arb/nob)', () => {
    expect(detectLanguage(CHINESE)).toBe('zh');
    expect(detectLanguage(ARABIC)).toBe('ar');
    expect(detectLanguage(NORWEGIAN)).toBe('no');
  });
});

describe('getLanguageName', () => {
  it('maps known ISO codes to display names', () => {
    expect(getLanguageName('en')).toBe('English');
    expect(getLanguageName('de')).toBe('German');
    expect(getLanguageName('zh')).toBe('Chinese');
  });

  it('falls back to the code itself when unknown', () => {
    expect(getLanguageName('xx')).toBe('xx');
    expect(getLanguageName('unknown')).toBe('unknown');
  });
});

describe('isCjkLanguage', () => {
  it('flags Chinese, Japanese, and Korean codes', () => {
    expect(isCjkLanguage('zh')).toBe(true);
    expect(isCjkLanguage('ja')).toBe(true);
    expect(isCjkLanguage('ko')).toBe(true);
  });

  it('ignores case and a region subtag', () => {
    expect(isCjkLanguage('ZH')).toBe(true);
    expect(isCjkLanguage('zh-Hans')).toBe(true);
    expect(isCjkLanguage('ja-JP')).toBe(true);
  });

  it('returns false for supported scripts and empty input', () => {
    expect(isCjkLanguage('en')).toBe(false);
    expect(isCjkLanguage('ru')).toBe(false);
    expect(isCjkLanguage('tr')).toBe(false);
    expect(isCjkLanguage('')).toBe(false);
    expect(isCjkLanguage(undefined)).toBe(false);
    expect(isCjkLanguage(null)).toBe(false);
  });
});

describe('detectLanguages', () => {
  it('reports both languages and no mismatch when they match', () => {
    const result = detectLanguages(ENGLISH, ENGLISH);
    expect(result.resume).toBe('en');
    expect(result.jobAd).toBe('en');
    expect(result.resumeName).toBe('English');
    expect(result.jobAdName).toBe('English');
    expect(result.mismatch).toBe(false);
  });

  it('flags a mismatch across different languages', () => {
    const result = detectLanguages(ENGLISH, GERMAN);
    expect(result.resume).toBe('en');
    expect(result.jobAd).toBe('de');
    expect(result.mismatch).toBe(true);
  });

  it('does not flag a mismatch when one side is unknown', () => {
    const result = detectLanguages(ENGLISH, 'too short');
    expect(result.jobAd).toBe('unknown');
    expect(result.mismatch).toBe(false);
  });
});
