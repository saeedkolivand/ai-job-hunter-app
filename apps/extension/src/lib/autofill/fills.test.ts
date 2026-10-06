/**
 * Unit tests for the assisted-autofill matcher/filler's POSITIVE paths
 * (apps/extension/src/lib/autofill.ts): which empty fields get filled from the
 * profile, the name-split flag and the attribute-only / aria-labelledby signals.
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts. We
 * build a real form in `document`, run the REAL implementation, and assert which
 * fields were filled and the summary.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { planAndFill, splitName } from '../autofill';
import {
  expectEmpty,
  expectValues,
  fill,
  labelled,
  PROFILE,
  resetDocument,
  setForm,
  val,
  withAttr,
} from './test-support';

afterEach(resetDocument);

describe('splitName', () => {
  it('splits first token vs remainder', () => {
    expect(splitName('Saeed Kolivand')).toEqual({ first: 'Saeed', last: 'Kolivand' });
    expect(splitName('Ana Maria De La Cruz')).toEqual({
      first: 'Ana',
      last: 'Maria De La Cruz',
    });
    expect(splitName('Cher')).toEqual({ first: 'Cher', last: '' });
    expect(splitName('   ')).toEqual({ first: '', last: '' });
  });
});

describe('planAndFill – fills matching empty fields', () => {
  it('fills email, full name, phone and linkedin from label/type signals', () => {
    const summary = fill(
      labelled([
        ['email', 'Email address', 'email'],
        ['name', 'Full name'],
        ['phone', 'Phone number', 'tel'],
        ['li', 'LinkedIn profile', 'url'],
      ])
    );

    expectValues({
      email: 'saeed@example.com',
      name: 'Saeed Kolivand',
      phone: '+31612345678',
      li: 'https://linkedin.com/in/saeed',
    });
    expect(summary.filledNothing).toBe(false);
    expect(summary.nameSplit).toBeNull(); // single full-name field, no split
  });

  it('fills via Tier-1 autocomplete tokens (email/url/city) and given/family split', () => {
    const summary = fill(
      withAttr('autocomplete', [
        ['e', 'email'],
        ['w', 'url'],
        ['city', 'address-level2'],
        ['gn', 'given-name'],
        ['fam', 'family-name'],
      ])
    );

    expectValues({
      e: 'saeed@example.com',
      w: 'https://saeed.dev',
      city: 'Amsterdam, Netherlands',
      gn: 'Saeed',
      fam: 'Kolivand',
    });
    // given/family came from splitting the full name → flagged.
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
  });

  it('dispatches an input event so framework-controlled inputs notice', () => {
    setForm(labelled([['email', 'Email', 'email']]));
    let fired = false;
    document.getElementById('email')!.addEventListener('input', () => {
      fired = true;
    });

    planAndFill(document, PROFILE);
    expect(fired).toBe(true);
  });

  it('counts multiple fields that receive the same value', () => {
    const summary = fill(
      `<input id="e1" autocomplete="email" />${labelled([['e2', 'Email address', 'email']])}`
    );
    const email = summary.filled.find((f) => f.key === 'email');
    expect(email?.count).toBe(2);
    expectValues({ e1: 'saeed@example.com', e2: 'saeed@example.com' });
  });
});

describe('planAndFill – name-split flag', () => {
  it('flags the split when separate first/last fields are filled', () => {
    const summary = fill(
      `<label for="first">First name</label><input id="first" />
       <label for="last">Last name</label><input id="last" />`
    );
    expectValues({ first: 'Saeed', last: 'Kolivand' });
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
    expect(summary.filled.map((f) => f.key).sort()).toEqual(['firstName', 'lastName']);
  });
});

describe('planAndFill – attribute-only name fields (no "first name" phrase anywhere)', () => {
  it('fills separate first/last boxes from underscore / camelCase / abbreviated NAME attributes', () => {
    // The overwhelmingly common real-world shape: no <label>, no autocomplete —
    // just a `name` attribute. All of these used to resolve to null (nothing
    // filled at all) because the patterns required a literal space.
    const summary = fill(
      withAttr('name', [
        ['a', 'first_name'],
        ['b', 'last_name'],
        ['c', 'firstName'],
        ['d', 'lastName'],
        ['e', 'fname'],
        ['f', 'lname'],
        ['g', 'job_application[first_name]'],
        ['h', 'job_application[last_name]'],
      ])
    );

    for (const id of ['a', 'c', 'e', 'g']) expect(val(id), id).toBe('Saeed');
    for (const id of ['b', 'd', 'f', 'h']) expect(val(id), id).toBe('Kolivand');
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
  });

  it('regression: HYPHENATED first/last fields no longer BOTH receive the full name', () => {
    // `-` is not a word character, so these matched the generic `\bname\b`
    // catch-all: "Saeed Kolivand" was written into every one of them, silently.
    fill(
      withAttr('name', [
        ['fn', 'first-name'],
        ['gn', 'given-name'],
        ['ln', 'last-name'],
        ['famn', 'family-name'],
      ])
    );

    expectValues({ fn: 'Saeed', gn: 'Saeed', ln: 'Kolivand', famn: 'Kolivand' });
  });

  it("never writes the user's name into a THIRD PARTY's name box", () => {
    // The attribute spellings out-ran the denylist, which is prose-shaped
    // (`AMBIGUOUS`) or leading-anchored (`AMBIGUOUS_PREFIXED`) — so every box
    // below silently received the applicant's own name.
    const names = [
      'professionalReferenceFirstName',
      'jobReferenceFirstName',
      'myReferrerFirstName',
      'spouseFirstName',
      'dependentLastName',
      'beneficiaryFirstName',
      'previousLastName',
      'aka_first_name',
      'childFirstName',
      'otherLastName',
    ];
    const summary = fill(
      withAttr(
        'name',
        names.map((name, i) => [`t${i + 1}`, name])
      )
    );

    expectEmpty(...names.map((_, i) => `t${i + 1}`));
    expect(summary.filledNothing).toBe(true);
  });

  it('skips a middle-name box instead of giving it the full name', () => {
    // There is no middleName in the profile; `middle-name` used to reach the
    // bare-"name" catch-all and receive "Saeed Kolivand".
    fill(
      withAttr('name', [
        ['m1', 'first-name'],
        ['m2', 'middle-name'],
        ['m3', 'middle_name'],
        ['m4', 'last-name'],
      ])
    );

    expectValues({ m1: 'Saeed', m2: '', m3: '', m4: 'Kolivand' });
  });

  it('does not let the fullName attribute spellings escape the school/company denylist', () => {
    fill(
      withAttr('name', [
        ['s1', 'school_full_name'],
        ['s2', 'universityFullName'],
        ['s3', 'degreeFullName'],
        ['s4', 'courseFullName'],
      ])
    );
    expectEmpty('s1', 's2', 's3', 's4');
  });

  it('still refuses a username / user_name login field', () => {
    fill(
      withAttr('name', [
        ['u1', 'username'],
        ['u2', 'user_name'],
        ['u3', 'fullName'],
      ])
    );

    // …while the full-name attribute spelling next to them still fills.
    expectValues({ u1: '', u2: '', u3: 'Saeed Kolivand' });
  });
});

describe('planAndFill – aria-labelledby labels (Workday / Ashby)', () => {
  it('resolves an aria-labelledby id LIST into the field signal', () => {
    fill(`
      <div id="lbl-first">First name</div>
      <input id="wf" aria-labelledby="lbl-first" />
      <div id="lbl-email">Email address</div>
      <div id="lbl-req">(required)</div>
      <input id="we" type="email" aria-labelledby="lbl-email lbl-req" />
    `);

    expectValues({ wf: 'Saeed', we: 'saeed@example.com' });
  });

  it('fills the WHOLE name into a single box placeheld "First and Last Name"', () => {
    // Prose names both halves; only the field's own ATTRIBUTE may veto the
    // fullName row. Reading the placeholder as attribute-style evidence made
    // this box fall through to lastName and receive just "Kolivand".
    fill(`
      <label for="fn1">Full Name</label>
      <input id="fn1" placeholder="First and Last Name" />
      <label for="fn2">First and Last Name</label><input id="fn2" />
      <input id="fn3" aria-label="First &amp; last name" />
    `);

    expectValues({ fn1: 'Saeed Kolivand', fn2: 'Saeed Kolivand', fn3: 'Saeed Kolivand' });
  });

  it("lets each box's own attribute win over a shared 'Full Name' GROUP label", () => {
    // Workday/Ashby point every box of a group at one heading, so "Full Name"
    // reaches the first/last inputs' signals — and the fullName row runs first.
    fill(`
      <span id="grp">Full Name</span>
      <input id="gf" name="first_name" aria-labelledby="grp" />
      <input id="gl" name="last_name" aria-labelledby="grp" />
    `);

    expectValues({ gf: 'Saeed', gl: 'Kolivand' });
  });

  it('ignores an aria-labelledby that points at a missing id', () => {
    fill(`<input id="ghost" aria-labelledby="does-not-exist" />`);
    expectEmpty('ghost');
  });

  it('applies the ambiguous denylist to an aria-labelledby label too', () => {
    fill(`
      <div id="lbl-emg">Emergency contact phone</div>
      <input id="emg" type="tel" aria-labelledby="lbl-emg" />
    `);
    expectEmpty('emg');
  });
});
