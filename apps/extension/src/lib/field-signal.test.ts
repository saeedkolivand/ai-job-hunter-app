/**
 * Unit tests for the named-key matcher (apps/extension/src/lib/field-signal.ts).
 *
 * `matchNamedKey` is a pure signal → key function, so these tests drive it with
 * the raw signal strings `textSignal` produces (name + id + placeholder +
 * aria-label + label, diacritic-stripped and lowercased).
 *
 * The focus is the module's stated contract: it must **under-fill rather than
 * mis-fill**. A field that resolves to the wrong key writes the user's PII into
 * a visibly wrong box, which they may not notice and cannot undo.
 */

import { describe, expect, it } from 'vitest';

import { isAmbiguousSignal, matchNamedKey } from './field-signal';

describe('isAmbiguousSignal', () => {
  it('still skips the genuinely ambiguous / sensitive fields', () => {
    for (const signal of [
      'referral source',
      'referrer',
      'job_referral',
      'professional references',
      'reference_name',
      'site search',
      'job_search',
      'emergency contact',
      'confirm password',
      'company name',
      'recruiter',
      'ssn',
      'passport number',
      // \b-anchored short terms, unchanged
      'dni',
      "contact d'urgence",
    ]) {
      expect(isAmbiguousSignal(signal), signal).toBe(true);
    }
  });

  it('does not skip a field that merely CONTAINS a denylist term', () => {
    // `referr` ⊂ "preferred", `search` ⊂ "research", `reference` ⊂ "preferences".
    // These were skipped entirely — never filled AND never captured.
    for (const signal of [
      'preferred first name',
      'preferred name',
      'preferred pronouns',
      'research experience',
      'research interests',
      'work preferences',
      'notification preferences',
    ]) {
      expect(isAmbiguousSignal(signal), signal).toBe(false);
    }
  });

  it('lets a freed-up field resolve to its real key', () => {
    // "Preferred first name" is ubiquitous on ATS forms; once it is no longer
    // treated as ambiguous it fills as a first name.
    expect(matchNamedKey('preferred first name')).toBe('firstName');
  });
});

describe('matchNamedKey — phone', () => {
  it('matches real phone fields, including the separator-less compounds', () => {
    for (const signal of [
      'phone',
      'phone number',
      'phonenumber',
      'phone_number',
      'primary phone',
      'candidate_phone',
      'cell phone',
      'cellphone',
      'work phone',
      'home phone',
      'mobile',
      'mobile_number',
      'mobilenumber',
      'telephone',
      'telefonnummer',
      'telefoon',
      'puhelin',
    ]) {
      expect(matchNamedKey(signal), signal).toBe('phone');
    }
  });

  it('does not match a word that merely CONTAINS phone/mobile', () => {
    // Bare `phone`/`mobile` were unanchored substrings, so a "Smartphone model"
    // field resolved to `phone` and received the user's phone number.
    for (const signal of ['smartphone', 'smartphone model', 'iphone', 'microphone', 'automobile']) {
      expect(matchNamedKey(signal), signal).not.toBe('phone');
    }
  });
});

describe('matchNamedKey — location', () => {
  it('matches real city/town fields, including the separator-less compounds', () => {
    for (const signal of [
      'city',
      'city name',
      'cityname',
      'candidate_city',
      'city_1',
      'current city',
      'town',
      'hometown',
      'location',
      'wohnort',
      'ciudad',
      'plaats',
      'miasto',
      'cidade',
      'localidad',
    ]) {
      expect(matchNamedKey(signal), signal).toBe('location');
    }
  });

  it('does not match a word that merely CONTAINS city', () => {
    // `city` ⊂ "ethnicity": an EEO ethnicity field used to resolve to `location`
    // and be filled with the user's city.
    for (const signal of ['ethnicity', 'ethnicity / race', 'race and ethnicity']) {
      expect(matchNamedKey(signal), signal).not.toBe('location');
    }
  });
});
