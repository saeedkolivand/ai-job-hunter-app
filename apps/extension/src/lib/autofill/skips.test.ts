/**
 * Unit tests for what assisted autofill must NOT touch (apps/extension/src/lib/
 * autofill.ts): ambiguous / sensitive / hidden / already-filled / disabled
 * fields. The matcher's contract is to under-fill rather than mis-fill.
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts.
 */

import { afterEach, describe, expect, it } from 'vitest';

import { hasAutofillableFields } from '../autofill';
import {
  addStyle,
  expectEmpty,
  expectValues,
  fill,
  labelled,
  resetDocument,
  setForm,
  val,
} from './test-support';

afterEach(resetDocument);

describe('planAndFill – skips ambiguous / sensitive / hidden / filled', () => {
  /** Cases where every listed field must stay empty: `[name, html, emptyIds, css?, profile?]`. */
  it.each([
    [
      'skips password, hidden, and search inputs',
      `
      <input id="pw" type="password" autocomplete="email" />
      <input id="hid" type="hidden" autocomplete="email" />
      <label for="s">Search jobs</label><input id="s" type="search" />`,
      ['pw', 'hid', 's'],
    ],
    [
      'skips a credit-card autocomplete token',
      `<input id="cc" autocomplete="cc-number" />`,
      ['cc'],
    ],
    [
      'skips ambiguous labels (referrer, company, confirm, emergency, manager)',
      labelled([
        ['ref', 'Referrer email', 'email'],
        ['co', 'Company website', 'url'],
        ['ce', 'Confirm email', 'email'],
        ['em', 'Emergency phone', 'tel'],
        ['mgr', 'Manager name'],
      ]),
      ['ref', 'co', 'ce', 'em', 'mgr'],
    ],
    [
      'never touches a textarea (cover letter / why this role)',
      `<label for="cl">Why this role</label><textarea id="cl" autocomplete="email"></textarea>`,
      ['cl'],
    ],
    [
      'skips a field hidden by an ancestor display:none',
      `<div style="display:none"><input id="dn" autocomplete="email" /></div>`,
      ['dn'],
    ],
    [
      // Real anti-bot honeypots (Greenhouse/Lever/Workday) hide the trap field via an
      // external-stylesheet / <style> utility class, never an inline style="display:none" —
      // an inline-only check would miss this and fill (and thus flag) the honeypot.
      'skips a field hidden by an ancestor CSS CLASS (honeypot), not just inline style',
      `<div class="ajh-visually-hidden"><input id="hp" autocomplete="email" /></div>`,
      ['hp'],
      '.ajh-visually-hidden { display: none; }',
    ],
    [
      'does not mis-fill education "Name" fields (School/University/Degree) with the full name',
      labelled([
        ['school', 'School Name'],
        ['uni', 'University Name'],
        ['deg', 'Degree Name'],
        ['course', 'Course Name'],
      ]),
      ['school', 'uni', 'deg', 'course'],
    ],
    [
      'skips sensitive PII fields (SSN, passport, date of birth) even though the matcher never targets them',
      `
      <label for="ssn">SSN</label><input id="ssn" type="text" autocomplete="email" />
      <label for="pp">Passport number</label><input id="pp" type="text" autocomplete="email" />
      <label for="dob">Date of birth</label><input id="dob" type="text" autocomplete="email" />`,
      ['ssn', 'pp', 'dob'],
    ],
    [
      'skips a "Driver\'s license number" field even though the matcher never targets it',
      `<label for="dl">Driver's license number</label><input id="dl" type="text" autocomplete="email" />`,
      ['dl'],
    ],
    [
      'does not map structured address sub-parts (street) from a single location string',
      `<input id="street" autocomplete="street-address" />`,
      ['street'],
    ],
    [
      'leaves a matched field empty when the profile has no value for it',
      labelled([['gh', 'GitHub', 'url']]),
      ['gh'],
      undefined,
      { email: 'x@y.z' }, // no github in profile
    ],
  ] as const)('%s', (_name, html, emptyIds, css, profile) => {
    if (css) addStyle(css);
    fill(html, profile as never);
    expectEmpty(...emptyIds);
  });

  it('never overwrites an already-filled field', () => {
    fill(`<label for="email">Email</label><input id="email" type="email" value="keep@me.com" />`);
    expect(val('email')).toBe('keep@me.com');
  });

  it('skips a field hidden by an ancestor with opacity:0 (honeypot), but still fills a normal sibling', () => {
    addStyle('.ajh-opacity-trap { opacity: 0; }');

    fill(`
      <div class="ajh-opacity-trap"><input id="op" autocomplete="email" /></div>
      ${labelled([['normal', 'Email', 'email']])}
    `);
    // Guard against false positives: a normal visible field must still fill.
    expectValues({ op: '', normal: 'saeed@example.com' });
  });

  it('skips a field shoved off-screen via position:absolute + left:-9999px (honeypot), but still fills a normal sibling', () => {
    fill(`
      <div style="position:absolute; left:-9999px;"><input id="off" autocomplete="email" /></div>
      ${labelled([['normal2', 'Email', 'email']])}
    `);
    // Guard against false positives: a normal visible field must still fill.
    expectValues({ off: '', normal2: 'saeed@example.com' });
  });

  it('under-fills: a bare "Website" is skipped, but "Portfolio" is filled', () => {
    fill(
      labelled([
        ['w1', 'Website', 'url'],
        ['w2', 'Portfolio URL', 'url'],
      ])
    );
    // ambiguous bare "Website" → under-fill
    expectValues({ w1: '', w2: 'https://saeed.dev' });
  });

  it('does not fill an X / Twitter field when the profile has no extra links (no twitter slot — #1218)', () => {
    // `valueForKey` yields '' for the twitter key (no profile slot), so an
    // X/Twitter handle box is never given a NAMED value. With no extraLinks in
    // the profile the field is left untouched (with a matching extra link it
    // IS filled — see the Tier-2 extra-link suite). Email still fills.
    const summary = fill(
      labelled([
        ['x', 'X / Twitter'],
        ['tw', 'Twitter handle'],
        ['email', 'Email', 'email'],
      ])
    );
    expectValues({ x: '', tw: '', email: 'saeed@example.com' });
    expect(summary.filledNothing).toBe(false);
  });
});

describe('planAndFill – disabled / readonly fields', () => {
  it('never writes into a disabled or readonly field', () => {
    const summary = fill(`
      <label for="d1">Email</label><input id="d1" type="email" disabled />
      <label for="r1">Full name</label><input id="r1" type="text" readonly />
      ${labelled([['ok1', 'Phone number', 'tel']])}
    `);

    // Guard against a false positive: a normal sibling still fills, and the
    // summary counts ONLY what was really written.
    expectValues({ d1: '', r1: '', ok1: '+31612345678' });
    expect(summary.filled.map((f) => f.key)).toEqual(['phone']);
  });

  it('skips a field disabled by an ancestor <fieldset disabled>', () => {
    // `el.disabled` reflects only the element's OWN attribute, so the property
    // check alone would fill (and count) a whole disabled section.
    const summary = fill(`
      <fieldset disabled>
        <label for="fs1">Email</label><input id="fs1" type="email" />
        <input id="fs2" name="first_name" />
      </fieldset>
      ${labelled([['live', 'Phone number', 'tel']])}
    `);

    expectValues({ fs1: '', fs2: '', live: '+31612345678' });
    expect(summary.filled.map((f) => f.key)).toEqual(['phone']);
  });

  it('does not report a page of only disabled/readonly fields as autofillable', () => {
    setForm(`
      <label for="d2">Email</label><input id="d2" type="email" disabled />
      <label for="r2">First name</label><input id="r2" type="text" readonly />
    `);
    expect(hasAutofillableFields(document)).toBe(false);
  });
});

describe('planAndFill – filled-nothing', () => {
  it('reports filledNothing when no field matches', () => {
    const summary = fill(`
      <input id="pw" type="password" />
      <label for="s">Search</label><input id="s" type="search" />
      <textarea id="ta"></textarea>
    `);
    expect(summary.filledNothing).toBe(true);
    expect(summary.filled).toHaveLength(0);
  });
});
