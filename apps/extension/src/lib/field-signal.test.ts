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
      // camelCase compound flattens to `workphone`; `work` is an enumerated
      // prefix, so it still resolves to phone (see NAMED_KEY_PATTERNS note).
      'workphone',
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
    // field resolved to `phone` and received the user's phone number. `headphone`
    // has a non-enumerated `head` prefix, so it stays a non-match too.
    for (const signal of [
      'smartphone',
      'smartphone model',
      'iphone',
      'microphone',
      'headphone',
      'automobile',
    ]) {
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
      // camelCase compounds flatten to `workcity` / `homecity`; `home`/`work` are
      // enumerated prefixes, so they resolve to location (see NAMED_KEY_PATTERNS).
      'workcity',
      'homecity',
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

describe('matchNamedKey — first / last name', () => {
  it('matches the attribute spellings of a first-name field, not just the "first name" phrase', () => {
    for (const signal of [
      'first name',
      // camelCase `firstName` flattens to `firstname` in the lowercased signal.
      'firstname',
      'first_name',
      'first-name',
      'job_application[first_name]', // Greenhouse
      'fname',
      'fname_1',
      'first_nm', // Taleo
      'firstnm',
      'given name',
      'given-name',
      'givenname',
      'forename',
      'candidate_first_name',
      // No separator at all before `first` (camelCase `applicantFirstName`) —
      // these patterns are deliberately NOT leading-anchored (nothing collides).
      'applicantfirstname',
      'preferred first name',
      'vorname',
      'prenom',
    ]) {
      expect(matchNamedKey(signal), signal).toBe('firstName');
    }
  });

  it('matches the attribute spellings of a last-name field', () => {
    for (const signal of [
      'last name',
      'lastname',
      'last_name',
      'last-name',
      'job_application[last_name]', // Greenhouse
      'lname',
      'lnameinput',
      'last_nm',
      'lastnm',
      'family name',
      'family-name',
      'familyname',
      'surname',
      'candidate_last_name',
      'applicantlastname',
      'nachname',
      'nazwisko',
    ]) {
      expect(matchNamedKey(signal), signal).toBe('lastName');
    }
  });

  it('regression: a HYPHENATED first/last field no longer falls through to the bare-name catch-all', () => {
    // `-` is not a word character, so `first-name` / `family-name` used to match
    // the generic `\bname\b` catch-all and receive the FULL name in BOTH boxes —
    // a silent mis-fill, the one failure mode this module exists to prevent.
    expect(matchNamedKey('first-name')).toBe('firstName');
    expect(matchNamedKey('given-name')).toBe('firstName');
    expect(matchNamedKey('last-name')).toBe('lastName');
    expect(matchNamedKey('family-name')).toBe('lastName');
  });

  it('keeps full-name fields on fullName — including the attribute spellings', () => {
    for (const signal of [
      'full name',
      'fullname',
      'full_name',
      'full-name',
      'candidatefullname',
      // The bare-"name" catch-all is unchanged and still runs LAST.
      'name',
      'your name',
      'vollstandiger name',
      'nombre completo',
      'imie i nazwisko',
    ]) {
      expect(matchNamedKey(signal), signal).toBe('fullName');
    }
  });

  it('does not let `lname` claim `fullname` (ordering + leading anchor)', () => {
    // `lname` ⊂ "fu**llname**": unanchored it would turn every `fullName` field
    // into a lastName one.
    expect(matchNamedKey('fullname')).toBe('fullName');
    expect(matchNamedKey('candidate_fullname')).toBe('fullName');
  });

  it('never routes a username-family / non-person "name" field to a person key', () => {
    // `username`/`user name` are stopped by the denylist BEFORE matchNamedKey
    // runs (both autofill's `isCandidateField` and capture's `isCapturable`
    // check it first) …
    expect(isAmbiguousSignal('username')).toBe(true);
    expect(isAmbiguousSignal('user name')).toBe(true);
    // … and matchNamedKey itself must refuse them too, so widening the name
    // patterns can never write the user's name into a login field.
    for (const signal of [
      'username',
      'user name',
      'user_name',
      'nickname',
      'display name',
      'file name',
      'screen name',
      'school name',
      'university name',
    ]) {
      expect(matchNamedKey(signal), signal).toBeNull();
    }
  });
});
