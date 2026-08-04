import { describe, expect, it } from 'vitest';

import fixture from '../../fixtures/header-contact-line.json';
import sectionNames from '../../fixtures/section-names.json';
import {
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

  it('does NOT recognize a job-title line — the ALL-CAPS heuristic this replaces used to', () => {
    // The exact repro from the security review: the prompt mandates "Line 2:
    // Job title", and an ALL-CAPS title must never end the header-seeding scan.
    expect(isKnownSectionName('SENIOR SOFTWARE ENGINEER')).toBe(false);
    expect(isKnownSectionName('Jane Doe')).toBe(false);
  });
});
