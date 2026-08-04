import { describe, expect, it } from 'vitest';

import allCapsHeadings from '../../fixtures/all-caps-headings.json';
import fixture from '../../fixtures/header-contact-line.json';
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
  // No "every fixture name resolves true" loop here: isKnownSectionName's
  // Set is built directly FROM this same fixture (`const KNOWN_SECTION_NAMES
  // = new Set(sectionNames)`), so that loop would test the JSON parser, not
  // the predicate — it cannot fail. The real cross-implementation guard is
  // Rust's `section_names_exactly_matches_ts_known_section_names_fixture`,
  // which compares the fixture against its OWN independent `SECTION_NAMES`
  // list. The behavioral assertions below (case/whitespace/markdown
  // handling, exclusions) are what can actually break.
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

  // The es/it/nl/pt locale headers (PERFIL, FORMACIÓN, HABILIDADES,
  // ESPERIENZA PROFESSIONALE, …) previously lived here as a second,
  // TS-only hardcoded array — checked against nothing but this predicate,
  // so the Rust side never verified them. They now live in
  // fixtures/all-caps-headings.json instead, covered by both this fixture
  // test and `cargo test export::parser`'s copy — one list, checked twice.
});
