/**
 * Unit tests for the Tier-2 extra-link matcher (apps/extension/src/lib/autofill.ts):
 * a field whose label unambiguously token-matches one of the profile's
 * `extraLinks` is filled from it — and stays untouched when it is ambiguous,
 * generic, filled, hidden, or not URL-typed.
 */

import { afterEach, describe, expect, it } from 'vitest';

import type { AutofillProfile } from '../autofill';
import { expectEmpty, expectValues, fill, labelled, PROFILE, resetDocument } from './test-support';

afterEach(resetDocument);

// `website` deliberately unset: a "Portfolio"-labelled field maps to the
// generic `website` key first (the pre-existing heuristic), and only falls
// through to the extra-link matcher when that named slot is empty — see
// the `planAndFill` doc comment.
const PROFILE_WITH_LINKS: AutofillProfile = {
  ...PROFILE,
  website: undefined,
  extraLinks: [
    { label: 'Portfolio', url: 'https://saeed.dev/work' },
    { label: 'Dribbble', url: 'https://dribbble.com/saeed' },
  ],
};

/** A profile that holds nothing but these extra links. */
const linksOnly = (...links: [label: string, url: string][]): AutofillProfile => ({
  extraLinks: links.map(([label, url]) => ({ label, url })),
});

/** One `type=url` field labelled `label`. */
const urlField = (id: string, label: string, type = 'url') => labelled([[id, label, type]]);

describe('planAndFill – Tier-2 extra-link matching', () => {
  it('fills a field whose label unambiguously matches one extra link', () => {
    const summary = fill(urlField('p', 'Portfolio'), PROFILE_WITH_LINKS);
    expectValues({ p: 'https://saeed.dev/work' });
    expect(summary.filled).toContainEqual({
      key: 'extraLink:Portfolio',
      label: 'Portfolio',
      count: 1,
    });
  });

  it('matches case/diacritic-insensitively, as whole-word tokens (not a substring)', () => {
    fill(
      urlField('so', 'Stäck Overflöw profile'),
      linksOnly(['Stack Overflow', 'https://stackoverflow.com/users/1'])
    );
    expectValues({ so: 'https://stackoverflow.com/users/1' });
  });

  it('requires a whole-word token match, not a coincidental substring (e.g. "Dribbble" must not match "Dribbblers")', () => {
    fill(urlField('d', 'Dribbblers only'), linksOnly(['Dribbble', 'https://dribbble.com/saeed']));
    expectEmpty('d');
  });

  it('skips (ambiguous) a field whose signal matches MULTIPLE extra links, and flags it in the summary', () => {
    const summary = fill(urlField('both', 'Portfolio Dribbble'), PROFILE_WITH_LINKS);
    expectEmpty('both');
    expect(summary.skippedAmbiguous).toBe(1);
    expect(summary.filled).toHaveLength(0);
  });

  it('does NOT match a bare "Website" field label to an extra link literally labelled "Website"', () => {
    const summary = fill(
      urlField('w', 'Website'),
      linksOnly(['Website', 'https://saeed.dev/secondary'])
    );
    expectEmpty('w');
    expect(summary.skippedAmbiguous ?? 0).toBe(0);
    expect(summary.filledNothing).toBe(true);
  });

  it('never overwrites an already-filled field, even when its label matches a link', () => {
    fill(
      `<label for="p">Portfolio</label><input id="p" type="url" value="https://keep.me" />`,
      PROFILE_WITH_LINKS
    );
    expectValues({ p: 'https://keep.me' });
  });

  it('never fills a hidden (honeypot) field even when its label matches a link', () => {
    fill(`<div style="display:none">${urlField('hp', 'Portfolio')}</div>`, PROFILE_WITH_LINKS);
    expectEmpty('hp');
  });

  it('leaves a field with no matching link untouched', () => {
    const summary = fill(urlField('cl', 'Cover letter link'), PROFILE_WITH_LINKS);
    expectEmpty('cl');
    expect(summary.filledNothing).toBe(true);
  });

  it('a field filled by a named key WITH a value is never additionally reconsidered against extraLinks', () => {
    fill(urlField('li', 'LinkedIn profile'), {
      linkedin: 'https://linkedin.com/in/saeed',
      extraLinks: [{ label: 'LinkedIn Extra', url: 'https://example.com/other' }],
    });
    expectValues({ li: 'https://linkedin.com/in/saeed' });
  });

  it('does NOT fall through to the extra-link matcher for a non-website named key with an empty profile value (only `website` falls through)', () => {
    const summary = fill(
      urlField('li', 'LinkedIn'),
      linksOnly(['LinkedIn', 'https://linkedin.com/in/other'])
    );
    expectEmpty('li'); // named `linkedin` key claims it; no fallthrough
    expect(summary.filledNothing).toBe(true);
  });

  it('never fills an email/tel-typed field via the extra-link matcher (a URL is syntactically invalid there)', () => {
    const summary = fill(urlField('pe', 'Portfolio', 'email'), PROFILE_WITH_LINKS);
    expectEmpty('pe');
    expect(summary.filledNothing).toBe(true);
  });

  it('never matches a link labelled a bare "Profile" (GENERIC_LINK_LABELS)', () => {
    const summary = fill(
      urlField('prof', 'Profile'),
      linksOnly(['Profile', 'https://example.com/profile'])
    );
    expectEmpty('prof');
    expect(summary.filledNothing).toBe(true);
  });

  it('is a no-op when the profile has no extraLinks (absence tolerated)', () => {
    // "Dribbble" matches no existing Tier 1/2 named-key heuristic, so this
    // field is left untouched purely by the `links.length === 0` short-circuit.
    const summary = fill(urlField('d', 'Dribbble'));
    expectEmpty('d');
    expect(summary.skippedAmbiguous ?? 0).toBe(0);
  });

  it('token-normalizes the generic-label denylist against punctuation/hyphen variants', () => {
    // Matching is token-based, so a bare exact-string check on the denylist
    // (e.g. "website!" not literally in the set) would let these slip through
    // while still token-matching the plain field label.
    for (const [linkLabel, fieldLabel] of [
      ['Website!', 'Website'],
      ['Web-Site', 'Web Site'],
      ['Personal-Site', 'Personal Site'],
    ]) {
      const summary = fill(
        urlField('f', fieldLabel!),
        linksOnly([linkLabel!, 'https://example.com/x'])
      );
      expectEmpty('f');
      expect(summary.filledNothing).toBe(true);
    }
  });

  it('token-normalizes the generic-label denylist independent of word order (e.g. "Site Web" vs "Website")', () => {
    // The denylist comparison must be order-insensitive since the field
    // matcher itself is (tokens.every) — otherwise "Site Web" would bypass
    // the denylisted "web site" while still token-matching a "Website" field.
    const summary = fill(
      urlField('f', 'Website'),
      linksOnly(['Site Web', 'https://example.com/x'])
    );
    expectEmpty('f');
    expect(summary.filledNothing).toBe(true);
  });

  it('fills BOTH fields when two fields share one label and one matching link exists', () => {
    const summary = fill(
      labelled([
        ['p1', 'Portfolio', 'url'],
        ['p2', 'Portfolio', 'url'],
      ]),
      PROFILE_WITH_LINKS
    );
    expectValues({ p1: 'https://saeed.dev/work', p2: 'https://saeed.dev/work' });
    expect(summary.skippedAmbiguous ?? 0).toBe(0);
    expect(summary.filled).toContainEqual({
      key: 'extraLink:Portfolio',
      label: 'Portfolio',
      count: 2,
    });
  });

  it('matches a diacritic link label against an equivalent plain-ASCII field label (symmetry)', () => {
    // The reverse of the "Stäck Overflöw" case above: here the LINK carries
    // the diacritic and the FIELD is plain ASCII.
    const summary = fill(
      urlField('up', 'Uberprofil'),
      linksOnly(['Überprofil', 'https://example.com/uber'])
    );
    expectValues({ up: 'https://example.com/uber' });
    expect(summary.filled).toContainEqual({
      key: 'extraLink:Überprofil',
      label: 'Überprofil',
      count: 1,
    });
  });

  it('falls through to the extra-link matcher for an X / Twitter field with a matching link (#1218)', () => {
    // The `twitter` key has NO profile slot (valueForKey's default branch → ''),
    // so unlike linkedin/github the field must NOT be claimed-and-dropped: it
    // falls through to Tier 2 exactly like `website`, and a matching X/Twitter
    // extra link fills it — preserving the pre-#1218 fill behavior the
    // NAMED_KEY_PATTERNS row must not regress.
    const summary = fill(
      labelled([
        ['x', 'X', 'url'],
        ['tw', 'Twitter handle'],
      ]),
      {
        ...PROFILE,
        extraLinks: [
          { label: 'X', url: 'https://x.com/saeed' },
          { label: 'Twitter', url: 'https://twitter.com/saeed' },
        ],
      }
    );
    expectValues({ x: 'https://x.com/saeed', tw: 'https://twitter.com/saeed' });
    expect(summary.filled).toContainEqual({ key: 'extraLink:X', label: 'X', count: 1 });
    expect(summary.filled).toContainEqual({ key: 'extraLink:Twitter', label: 'Twitter', count: 1 });
    expect(summary.filledNothing).toBe(false);
  });

  it('leaves an X / Twitter field untouched and unreported when no extra link matches (#1218)', () => {
    const summary = fill(urlField('x', 'X'), {
      ...PROFILE,
      extraLinks: [{ label: 'Portfolio', url: 'https://saeed.dev/work' }],
    });
    expectEmpty('x');
    expect(summary.filledNothing).toBe(true);
    expect(summary.filled).toHaveLength(0);
  });

  it('fills an "X handle" field from a matching extra link (#1218)', () => {
    // An "X handle" label now resolves to the `twitter` key through the
    // x+qualifier branch of the same row — and like the bare "X" field above,
    // the empty twitter slot falls through to the extra-link matcher, so the
    // field fills from a link labelled "X" (token-based matching: the {x,
    // handle} field tokens line up with the label's `x`).
    const summary = fill(urlField('xh', 'X handle', 'text'), {
      ...PROFILE,
      extraLinks: [{ label: 'X', url: 'https://x.com/saeed' }],
    });
    expectValues({ xh: 'https://x.com/saeed' });
    expect(summary.filled).toContainEqual({ key: 'extraLink:X', label: 'X', count: 1 });
    expect(summary.filledNothing).toBe(false);
  });
});
