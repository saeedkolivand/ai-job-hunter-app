import { describe, expect, it } from 'vitest';

import allCapsHeadings from '../../fixtures/all-caps-headings.json';
import fixture from '../../fixtures/header-contact-line.json';
import sectionNames from '../../fixtures/section-names.json';
import {
  isAllCapsSectionHeading,
  isFirstLineContactShaped,
  isHeaderContactLine,
  isKnownSectionName,
} from './header-contact-line';

describe('isHeaderContactLine / isFirstLineContactShaped ↔ Rust parity', () => {
  // Shared source of truth with `cargo test export::parser` — both suites read
  // fixtures/header-contact-line.json so the two implementations can never
  // silently drift.
  it('matches the shared fixture for every line', () => {
    const cases = fixture as { line: string; contact: boolean; firstLine: boolean }[];
    expect(cases.length).toBeGreaterThan(0);
    for (const { line, contact, firstLine } of cases) {
      expect(isHeaderContactLine(line)).toBe(contact);
      expect(isFirstLineContactShaped(line)).toBe(firstLine);
    }
  });
});

describe('isKnownSectionName', () => {
  it('recognizes every name in the shared fixture (its own runtime data source)', () => {
    const names = sectionNames as string[];
    expect(names.length).toBeGreaterThan(0);
    for (const name of names) {
      expect(isKnownSectionName(name)).toBe(true);
    }
  });

  it('is case-insensitive and trims whitespace', () => {
    expect(isKnownSectionName('EXPERIENCE')).toBe(true);
    expect(isKnownSectionName('  Experience  ')).toBe(true);
    expect(isKnownSectionName('**Skills**')).toBe(true);
  });

  it("does NOT recognize a job-title line — that is isAllCapsSectionHeading's job, with its own exclusion", () => {
    expect(isKnownSectionName('SENIOR SOFTWARE ENGINEER')).toBe(false);
    expect(isKnownSectionName('Jane Doe')).toBe(false);
  });
});

describe('isAllCapsSectionHeading ↔ Rust parity', () => {
  // Shared source of truth with `cargo test export::parser` — both suites read
  // fixtures/all-caps-headings.json. A previous version of this predicate was
  // deleted from the TS mirror without a fixture gate, which silently broke
  // header-seeding for es/it/nl/pt résumés and any en résumé whose first
  // heading wasn't literally in SECTION_NAMES — this fixture is the guard
  // against that mistake recurring.
  it('matches the shared fixture for every line', () => {
    const cases = allCapsHeadings as { line: string; heading: boolean }[];
    expect(cases.length).toBeGreaterThan(0);
    for (const { line, heading } of cases) {
      expect(isAllCapsSectionHeading(line)).toBe(heading);
    }
  });

  it('recognizes every locale header CONVENTIONS ships, uppercased (the exact CRITICAL repro)', () => {
    // packages/prompts/src/locale/index.ts CONVENTIONS — es/it/nl/pt headers,
    // as the résumé prompt actually emits them (`.toUpperCase()`).
    const localeHeaders = [
      'PERFIL', // es/pt summary
      'EXPERIENCIA PROFESIONAL', // es experience
      'FORMACIÓN', // es education
      'HABILIDADES', // es skills
      'PROFILO', // it summary
      'ESPERIENZA PROFESSIONALE', // it experience
      'FORMAZIONE', // it education
      'COMPETENZE', // it skills
      'PROFIEL', // nl summary
      'WERKERVARING', // nl experience
      'OPLEIDING', // nl education
      'VAARDIGHEDEN', // nl skills
      'EXPERIÊNCIA PROFISSIONAL', // pt experience
      'FORMAÇÃO', // pt education
      'COMPETÊNCIAS', // pt skills
    ];
    for (const header of localeHeaders) {
      expect(isAllCapsSectionHeading(header)).toBe(true);
    }
  });
});
