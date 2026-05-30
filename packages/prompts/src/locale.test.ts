import { describe, expect, it } from 'vitest';

import { detectSections, estimateTokens } from './context-manager';
import { charsPerToken, resumeConventions } from './locale';

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
