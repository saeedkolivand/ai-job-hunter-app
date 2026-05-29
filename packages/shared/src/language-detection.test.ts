import { describe, expect, it } from 'vitest';

import { detectLanguage, detectLanguages, getLanguageName } from './language-detection';

// Reasonably long samples — franc needs ~enough text to be confident.
const ENGLISH =
  'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';
const GERMAN =
  'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';
const FRENCH =
  "Ingénieur logiciel expérimenté possédant une solide expérience dans la création d'applications web évolutives et de systèmes backend distribués pour de grandes organisations.";

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
