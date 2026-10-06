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
 *
 * Signal tables are written as one `|`-separated string per table (a signal never
 * contains `|`), so each table is a line instead of one line per signal.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { isAmbiguousSignal, labelText, matchNamedKey } from './field-signal';

/** Every signal in the `|`-separated `table` resolves to exactly `key`. */
function expectKey(table: string, key: string | null): void {
  for (const signal of table.split('|')) expect(matchNamedKey(signal), signal).toBe(key);
}

/** No signal in the table resolves to `key` (it may resolve to another key or none). */
function expectNotKey(table: string, key: string): void {
  for (const signal of table.split('|')) expect(matchNamedKey(signal), signal).not.toBe(key);
}

function expectAmbiguous(table: string, ambiguous: boolean): void {
  for (const signal of table.split('|')) {
    expect(isAmbiguousSignal(signal), signal).toBe(ambiguous);
  }
}

describe('isAmbiguousSignal', () => {
  it('still skips the genuinely ambiguous / sensitive fields', () => {
    // The last two are \b-anchored short terms, unchanged.
    expectAmbiguous(
      "referral source|referrer|job_referral|professional references|reference_name|site search|job_search|emergency contact|confirm password|company name|recruiter|ssn|passport number|dni|contact d'urgence",
      true
    );
  });

  it('does not skip a field that merely CONTAINS a denylist term', () => {
    // `referr` ⊂ "preferred", `search` ⊂ "research", `reference` ⊂ "preferences".
    // These were skipped entirely — never filled AND never captured.
    expectAmbiguous(
      'preferred first name|preferred name|preferred pronouns|research experience|research interests|work preferences|notification preferences',
      false
    );
  });

  it('lets a freed-up field resolve to its real key', () => {
    // "Preferred first name" is ubiquitous on ATS forms; once it is no longer
    // treated as ambiguous it fills as a first name.
    expect(matchNamedKey('preferred first name')).toBe('firstName');
  });
});

describe('isAmbiguousSignal — third-party / non-fillable name COMPOUNDS', () => {
  it('skips a name that belongs to someone else — in attribute spellings too', () => {
    // The denylist is prose-shaped (`AMBIGUOUS`) or leading-anchored
    // (`AMBIGUOUS_PREFIXED`), so once the name patterns learned the attribute
    // spellings, every camelCase third-party box below started receiving the
    // USER's name. Skipping is the only correct answer for all of them.
    expectAmbiguous(
      'professionalreferencefirstname|jobreferencefirstname|proreferencelastname|workreferencelastname|myreferrerfirstname|staffreferralfirstname|refereelastname|spousefirstname|childfirstname|dependentlastname|beneficiaryfirstname|previouslastname|formerfirstname|otherlastname|aka_first_name|alsoknownasfirstname|also_known_as_last_name',
      true
    );
    // Taleo's `nm` abbreviation — the deny patterns end `n(?:ame|m)` for
    // exactly these, and the name PATTERNS match them, so both halves must
    // agree or a third party's `…Nm` box gets filled.
    expectAmbiguous('spousefirstnm|referencelastnm|dependentlastnm|beneficiaryfirstnm', true);
    // A leading `[^p]` character guard (the first attempt at exempting
    // "preferred") silently exempted EVERY p-terminated prefix — these are
    // ordinary HRIS spellings and each one was filled with the user's name.
    expectAmbiguous(
      'topreferencefirstname|groupreferencefirstname|helpreferencefirstname|backupreferencefirstname|signupreferencefirstname|stepreferencefirstname|shipreferencefirstname|campreferencefirstname|vipreferencefirstname|empreferencefirstname',
      true
    );
    // Already denied before this rule (leading-anchored / prose) — pinned so
    // the compound rule can never be "simplified" into losing them.
    expectAmbiguous(
      'reference_first_name|references[0][first_name]|referencefirstname|emergencycontactfirstname|mothers_maiden_name',
      true
    );
  });

  it('skips the name parts we hold no profile value for (middle / additional / kana)', () => {
    // There is no middleName key in the profile, so the ONLY safe outcome is a
    // skip — a hyphenated `middle-name` used to reach the bare-name catch-all
    // and receive the full name. The kana reading appears on BOTH sides of the
    // name token.
    expectAmbiguous(
      'middle name|middlename|middle_name|middle-name|middleinitial name|additional name|lastnamekana|name_kana|kana_last_name|kanalastname|furigana_first_name|furigana',
      true
    );
  });

  it('still lets the preferred/research/preferences family through (the `referr` ⊂ "preferred" trap)', () => {
    expectAmbiguous(
      'preferred first name|preferred_first_name|preferredfirstname|preferred name|preferred pronouns|work preferences|research experience|notification preferences',
      false
    );
    // …and the camelCase spelling resolves like the prose one.
    expect(matchNamedKey('preferredfirstname')).toBe('firstName');
    // The exemption covers ONLY the reference family, so a "preferred" that
    // sits next to another skip-stem does not un-deny the field.
    expect(isAmbiguousSignal('preferred middle name')).toBe(true);
    expect(isAmbiguousSignal('preferredspousefirstname')).toBe(true);
    // Documented collateral: "…dPreferred…" and "…pReference…" are spelled
    // alike apart from the word boundary, so the camelCase preferred-name field
    // is skipped rather than risk filling a reference's box.
    expect(isAmbiguousSignal('candidatepreferredfirstname')).toBe(true);
  });
});

describe('matchNamedKey — phone', () => {
  it('matches real phone fields, including the separator-less compounds', () => {
    // `workphone`: camelCase compound flattens; `work` is an enumerated prefix,
    // so it still resolves to phone (see NAMED_KEY_PATTERNS note).
    expectKey(
      'phone|phone number|phonenumber|phone_number|primary phone|candidate_phone|cell phone|cellphone|work phone|workphone|home phone|mobile|mobile_number|mobilenumber|telephone|telefonnummer|telefoon|puhelin',
      'phone'
    );
  });

  it('does not match a word that merely CONTAINS phone/mobile', () => {
    // Bare `phone`/`mobile` were unanchored substrings, so a "Smartphone model"
    // field resolved to `phone` and received the user's phone number. `headphone`
    // has a non-enumerated `head` prefix, so it stays a non-match too.
    expectNotKey('smartphone|smartphone model|iphone|microphone|headphone|automobile', 'phone');
  });
});

describe('matchNamedKey — location', () => {
  it('matches real city/town fields, including the separator-less compounds', () => {
    // camelCase compounds flatten to `workcity` / `homecity`; `home`/`work` are
    // enumerated prefixes, so they resolve to location (see NAMED_KEY_PATTERNS).
    expectKey(
      'city|city name|cityname|candidate_city|city_1|current city|workcity|homecity|town|hometown|location|wohnort|ciudad|plaats|miasto|cidade|localidad',
      'location'
    );
  });

  it('does not match a word that merely CONTAINS city', () => {
    // `city` ⊂ "ethnicity": an EEO ethnicity field used to resolve to `location`
    // and be filled with the user's city.
    expectNotKey('ethnicity|ethnicity / race|race and ethnicity', 'location');
  });
});

describe('matchNamedKey — first / last name', () => {
  it('matches the attribute spellings of a first-name field, not just the "first name" phrase', () => {
    // camelCase `firstName` flattens to `firstname`; `job_application[first_name]`
    // is Greenhouse, `first_nm` Taleo. `applicantfirstname` has no separator at
    // all before `first` — these patterns are deliberately NOT leading-anchored
    // (nothing collides).
    expectKey(
      'first name|firstname|first_name|first-name|job_application[first_name]|fname|fname_1|first_nm|firstnm|given name|given-name|givenname|forename|candidate_first_name|applicantfirstname|preferred first name|vorname|prenom',
      'firstName'
    );
  });

  it('matches the attribute spellings of a last-name field', () => {
    // `job_application[last_name]` is Greenhouse.
    expectKey(
      'last name|lastname|last_name|last-name|job_application[last_name]|lname|lnameinput|last_nm|lastnm|family name|family-name|familyname|surname|candidate_last_name|applicantlastname|nachname|nazwisko',
      'lastName'
    );
  });

  it('regression: a HYPHENATED first/last field no longer falls through to the bare-name catch-all', () => {
    // `-` is not a word character, so `first-name` / `family-name` used to match
    // the generic `\bname\b` catch-all and receive the FULL name in BOTH boxes —
    // a silent mis-fill, the one failure mode this module exists to prevent.
    expectKey('first-name|given-name', 'firstName');
    expectKey('last-name|family-name', 'lastName');
  });

  it('keeps full-name fields on fullName — including the attribute spellings', () => {
    // The bare-"name" catch-all is unchanged and still runs LAST.
    expectKey(
      'full name|fullname|full_name|full-name|candidatefullname|name|your name|vollstandiger name|nombre completo|imie i nazwisko',
      'fullName'
    );
  });

  it('does not let `lname` claim `fullname` (ordering + leading anchor)', () => {
    // `lname` ⊂ "fu**llname**": unanchored it would turn every `fullName` field
    // into a lastName one.
    expectKey('fullname|candidate_fullname', 'fullName');
  });

  it('never routes a username-family / non-person "name" field to a person key', () => {
    // `username`/`user name` are stopped by the denylist BEFORE matchNamedKey
    // runs (both autofill's `isCandidateField` and capture's `isCapturable`
    // check it first) …
    expectAmbiguous('username|user name', true);
    // … and matchNamedKey itself must refuse them too, so widening the name
    // patterns can never write the user's name into a login field.
    expectKey(
      'username|user name|user_name|nickname|display name|file name|screen name|school name|university name',
      null
    );
  });

  it('applies the school/company denylist to the fullName ATTRIBUTE spellings too', () => {
    // The row-level `deny` exists for exactly this: the prose spelling
    // ("University Name") has always been refused by the catch-all's denylist,
    // but `university_full_name` matched the fullName row BEFORE the catch-all
    // ever ran, so it escaped — and received the applicant's name.
    expectKey(
      'school_full_name|university_full_name|college_full_name|institution_full_name|program_full_name|course_full_name|schoolfullname|universityfullname|degreefullname|certificationfullname|coursefullname|majorfullname',
      null
    );
  });

  it('keeps a single box that asks for BOTH halves on fullName', () => {
    // Prose freely names both halves of a name; the `lastName` row's `last name`
    // matches such a label, so without the conjunction forms (and with the
    // first/last veto reading prose) the box received only the surname. The last
    // is label + placeholder, as one signal.
    for (const signal of [
      'first and last name',
      'first & last name',
      'first/last name',
      'first name and last name',
      'full name first and last name',
    ]) {
      expect(matchNamedKey(signal, ''), signal).toBe('fullName');
    }
  });

  it("lets a field's own first/last ATTRIBUTE out-specify a 'Full Name' GROUP label", () => {
    // Workday/Ashby wire a group heading to each box via aria-labelledby, so
    // "full name" lands in the signal of a `first_name` input — and the fullName
    // row runs first, which put the WHOLE name in the first-name box. The veto
    // reads the ATTRIBUTE signal (2nd arg), never the prose.
    expect(matchNamedKey('first_name full name', 'first_name')).toBe('firstName');
    expect(matchNamedKey('last_name full name', 'last_name')).toBe('lastName');
    expect(matchNamedKey('lname full name', 'lname')).toBe('lastName');
    // …and the SAME prose with no first/last attribute stays a full-name field.
    expect(matchNamedKey('full name first and last name', 'candidate_name')).toBe('fullName');
    // A real full-name field is unaffected…
    expect(matchNamedKey('fullname full name')).toBe('fullName');
    // …and so are the localized COMBINED phrases (they carry no first/last
    // attribute token, only prose).
    expectKey(
      'vor- und nachname|nombre y apellidos|nome e cognome|voor- en achternaam',
      'fullName'
    );
  });
});

describe('matchNamedKey — X / Twitter identity link (#1218)', () => {
  it('matches X / Twitter profile fields (`twitter` substring; the whole-signal `x`)', () => {
    // The single letter matches only when the ENTIRE signal is x tokens —
    // including the realistic bare-X shape, where the field echoes its own
    // x in the id/name (`<label for="x">X</label><input id="x">` →
    // `textSignal` = " x  x").
    expectKey(
      'twitter|twitter handle|twitter_url|twitter_handle|twitterhandle|twitter/x|x / twitter|x| x |x x| x  x ',
      'twitter'
    );
  });

  it('matches an `x` immediately paired with a handle-ish qualifier', () => {
    // The realistic longer-label shapes (#1218): a field can be labelled
    // "X handle" / "X username" / "X profile" — matched through x + the
    // qualifier rather than the whole-signal x above — plus the attribute
    // spellings `x_handle`/`x-handle` an id may use.
    expectKey(
      'x handle|x username|x profile|x url|x link|x id|x-handle|x_handle|x username field|what is your x handle',
      'twitter'
    );
  });

  it('never matches an `x` that has ANY company in the signal — prose, not a handle', () => {
    // `\bx\b` would be wrong here: a whole-word x inside an otherwise-named
    // signal is prose ("Mac OS X experience", "Do you use x?"), and matching
    // it would silently hide a genuine application question from capture.
    // The qualifier check is ADJACENCY-based, so an x followed by a word that
    // is NOT a handle-ish qualifier, or by nothing at all, stays prose:
    // "x experience" / "x ray" have the wrong next word, "do you use x" tails
    // the signal, and "tax id" hides its x inside "tax" where the word
    // boundary can never align.
    expectNotKey(
      'experience|years of experience|mac os x experience|x experience|x ray|xray|do you use x|list any x certifications|box|tax|tax id|next|text|xero|xavier|ex|expiration|external|axis|sexual orientation',
      'twitter'
    );
  });

  it('sits LAST: a signal naming a more specific key keeps that key', () => {
    // The twitter row is deliberately the weakest-evidence row, so a combined
    // signal like `email x` resolves to the specific key, never to the letter.
    expect(matchNamedKey('email x')).toBe('email');
    expect(matchNamedKey('linkedin')).toBe('linkedin');
    expect(matchNamedKey('github')).toBe('github');
    expect(matchNamedKey('portfolio')).toBe('website');
  });
});

describe('labelText', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  /** Render `html` and return `labelText` of the element with `id`. */
  function labelOf(html: string, id: string): string {
    document.body.innerHTML = html;
    return labelText(document.getElementById(id) as HTMLInputElement);
  }

  it('counts a label referenced BOTH by for= and aria-labelledby only once', () => {
    // The React-Aria / headless-UI shape. `answers-capture` persists this string
    // as the question key, so a duplicated label duplicates the stored question.
    const html = `
      <label for="q" id="q-label">Why this role?</label>
      <input id="q" aria-labelledby="q-label" />`;
    expect(labelOf(html, 'q').trim()).toBe('Why this role?');
  });

  it('joins DISTINCT aria-labelledby references in order, after the <label>', () => {
    const html = `
      <span id="g">Contact</span><span id="h">Email address</span>
      <input id="e" aria-labelledby="g h" />`;
    expect(labelOf(html, 'e').trim()).toBe('Contact Email address');
  });

  it('counts a wrapping label that also carries for= only once', () => {
    const html = `<label for="w">Notice period<input id="w" /></label>`;
    expect(labelOf(html, 'w').trim()).toBe('Notice period');
  });

  it('collapses the markup whitespace inside a label', () => {
    // The question text is persisted + sent over the bridge, so the same
    // question must not key differently because of source indentation.
    const html = `
      <label for="m">Why
          this     role?</label><input id="m" />`;
    expect(labelOf(html, 'm').trim()).toBe('Why this role?');
  });

  it('caps an aria-labelledby reference that points at a whole container', () => {
    const html = `
      <div id="card">${'very long boilerplate '.repeat(60)}</div>
      <input id="c" aria-labelledby="card" />`;
    expect(labelOf(html, 'c').length).toBeLessThanOrEqual(310);
  });
});
