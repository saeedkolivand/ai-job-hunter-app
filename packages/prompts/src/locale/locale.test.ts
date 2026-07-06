import { describe, expect, it } from 'vitest';

import { detectSections, estimateTokens } from '../context-manager';
import letterConventionsFixture from '../fixtures/letter-conventions.json';
import {
  charsPerToken,
  countryToCurrency,
  countryToMarket,
  hasLetterConventions,
  LETTER_MARKET_CONVENTIONS,
  letterConventions,
  resolveMarket,
  resumeConventions,
} from './index';

const GERMAN_RESUME = `Max Mustermann
max@example.de

Profil
Erfahrener Softwareentwickler mit 8 Jahren Erfahrung.

Berufserfahrung
Acme GmbH — Senior Entwickler (2019 - heute)
Migration auf Microservices geleitet.

Ausbildung
B.Sc. Informatik, TU München

Kenntnisse
TypeScript, React, Node.js`;

const FRENCH_RESUME = `Marie Dupont
marie@example.fr

Profil
Développeuse logicielle expérimentée.

Expérience professionnelle
Acme SARL — Développeuse senior (2019 - présent)
Migration vers les microservices.

Formation
Master en informatique, Université de Paris

Compétences
TypeScript, React, Node.js`;

describe('detectSections — locale-aware', () => {
  it('segments a German resume by its native headers', () => {
    const names = detectSections(GERMAN_RESUME).map((s) => s.name);
    expect(names).toContain('Summary'); // Profil
    expect(names).toContain('Experience'); // Berufserfahrung
    expect(names).toContain('Education'); // Ausbildung
    expect(names).toContain('Skills'); // Kenntnisse
  });

  it('segments a French resume by its native headers', () => {
    const names = detectSections(FRENCH_RESUME).map((s) => s.name);
    expect(names).toContain('Experience'); // Expérience professionnelle
    expect(names).toContain('Education'); // Formation
    expect(names).toContain('Skills'); // Compétences
  });

  it('does not collapse a non-English resume into a single blob', () => {
    expect(detectSections(GERMAN_RESUME).length).toBeGreaterThan(3);
    expect(detectSections(FRENCH_RESUME).length).toBeGreaterThan(3);
  });
});

describe('estimateTokens — per-locale factor', () => {
  it('counts more tokens for German than the English 4-chars/token baseline', () => {
    const text = 'a'.repeat(100);
    expect(estimateTokens(text, 'de')).toBeGreaterThan(estimateTokens(text));
    expect(charsPerToken('de')).toBeLessThan(charsPerToken('en'));
  });
});

describe('resumeConventions', () => {
  it('returns localized headers, with an English fallback for unknown locales', () => {
    expect(resumeConventions('de').headers.experience).toBe('Berufserfahrung');
    expect(resumeConventions('fr').headers.skills).toBe('Compétences');
    expect(resumeConventions('xx').headers.experience).toBe('Work Experience');
  });
});

describe('letter-conventions parity (TS const ↔ JSON fixture ↔ Rust)', () => {
  // The runtime const and the JSON fixture are the same data; the fixture is the
  // pivot the Rust renderer also mirrors (same pattern as url-labels). They must
  // never drift.
  it('the TS const equals the JSON fixture exactly', () => {
    expect(LETTER_MARKET_CONVENTIONS).toEqual(letterConventionsFixture.markets);
  });

  it('every market carries a non-empty salutation, sign-off and notes', () => {
    for (const [id, c] of Object.entries(LETTER_MARKET_CONVENTIONS)) {
      expect(c.country, id).toBeTruthy();
      expect(c.salutations.generic, id).toBeTruthy();
      expect(c.signoffs.length, id).toBeGreaterThan(0);
      expect(c.notes.length, id).toBeGreaterThan(10);
    }
  });
});

describe('letterConventions', () => {
  it('resolves a known market and the DACH salary/start-date inclusions', () => {
    const de = letterConventions('de');
    expect(de.country).toBe('Germany');
    expect(de.subjectLine).toEqual({ use: true, label: 'Betreff' });
    expect(de.inclusions.join(' ')).toMatch(/salary expectation/i);
    expect(de.inclusions.join(' ')).toMatch(/start date/i);
  });

  it('falls back to the international baseline for an unknown market', () => {
    expect(letterConventions('zz')).toBe(LETTER_MARKET_CONVENTIONS.intl);
    expect(letterConventions(undefined)).toBe(LETTER_MARKET_CONVENTIONS.intl);
    expect(hasLetterConventions('zz')).toBe(false);
    expect(hasLetterConventions('de')).toBe(true);
  });
});

describe('countryToMarket + resolveMarket', () => {
  it('splits the country-sensitive markets', () => {
    expect(countryToMarket('US')).toBe('us');
    expect(countryToMarket('GB')).toBe('uk');
    expect(countryToMarket('de')).toBe('de'); // case-insensitive
    expect(countryToMarket('AT')).toBe('at');
    expect(countryToMarket('CH')).toBe('ch');
    expect(countryToMarket('BR')).toBe('br');
    expect(countryToMarket('ZZ')).toBeUndefined();
  });

  it('prioritizes override → job country → brief country → language → intl', () => {
    // Override wins over everything.
    expect(resolveMarket({ override: 'uk', jobCountry: 'DE', targetLanguage: 'de' })).toBe('uk');
    // The IXOPAY case: English letter, German job → German market.
    expect(resolveMarket({ jobCountry: 'DE', targetLanguage: 'en' })).toBe('de');
    // Ad silent on country → research-brief HQ country fills the gap.
    expect(resolveMarket({ briefCountry: 'FR', targetLanguage: 'en' })).toBe('fr');
    // Nothing but a language → that language's default market.
    expect(resolveMarket({ targetLanguage: 'ja' })).toBe('jp');
    // English with no country stays neutral (intl), not US.
    expect(resolveMarket({ targetLanguage: 'en' })).toBe('intl');
    // Truly nothing → intl.
    expect(resolveMarket({})).toBe('intl');
    // An invalid override is ignored (falls through to the chain).
    expect(resolveMarket({ override: 'zz', jobCountry: 'US' })).toBe('us');
  });
});

describe('countryToCurrency', () => {
  it('grounds the salary-lookup currency in the job country, case-insensitively', () => {
    expect(countryToCurrency('DE')).toBe('EUR');
    expect(countryToCurrency('gb')).toBe('GBP');
    expect(countryToCurrency('US')).toBe('USD');
    expect(countryToCurrency('CA')).toBe('CAD');
    // Croatia joined the Eurozone 2023-01-01; Bulgaria joined 2026-01-01.
    expect(countryToCurrency('HR')).toBe('EUR');
    expect(countryToCurrency('BG')).toBe('EUR');
    // EU members outside the Eurozone.
    expect(countryToCurrency('HU')).toBe('HUF');
    expect(countryToCurrency('RO')).toBe('RON');
    expect(countryToCurrency('ZZ')).toBeUndefined();
    expect(countryToCurrency(undefined)).toBeUndefined();
  });
});
