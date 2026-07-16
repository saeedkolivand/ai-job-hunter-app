/**
 * Unit tests for the assisted-autofill matcher/filler (apps/extension/src/lib/autofill.ts).
 *
 * jsdom is provided by the vitest environment declared in vitest.config.ts. We
 * build a real form in `document`, run the REAL implementation, and assert which
 * fields were filled, which were skipped, the name-split flag, the summary, and
 * the in-page overlay.
 */

import { afterEach, describe, expect, it } from 'vitest';

import {
  AUTOFILL_GLOBAL,
  type AutofillProfile,
  hasAutofillableFields,
  planAndFill,
  renderSummaryOverlay,
  runAutofill,
  splitName,
} from './autofill';

const PROFILE: AutofillProfile = {
  fullName: 'Saeed Kolivand',
  email: 'saeed@example.com',
  phone: '+31612345678',
  location: 'Amsterdam, Netherlands',
  linkedin: 'https://linkedin.com/in/saeed',
  github: 'https://github.com/saeed',
  website: 'https://saeed.dev',
};

function setForm(html: string): void {
  document.body.innerHTML = `<form>${html}</form>`;
}

function val(id: string): string {
  return (document.getElementById(id) as HTMLInputElement).value;
}

afterEach(() => {
  document.body.innerHTML = '';
  // Clean up any <style> injected by a honeypot-CSS test (tagged for removal).
  document.head.querySelectorAll('style[data-ajh-test]').forEach((s) => s.remove());
});

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
    setForm(`
      <label for="email">Email address</label><input id="email" type="email" />
      <label for="name">Full name</label><input id="name" type="text" />
      <label for="phone">Phone number</label><input id="phone" type="tel" />
      <label for="li">LinkedIn profile</label><input id="li" type="url" />
    `);

    const summary = planAndFill(document, PROFILE);

    expect(val('email')).toBe('saeed@example.com');
    expect(val('name')).toBe('Saeed Kolivand');
    expect(val('phone')).toBe('+31612345678');
    expect(val('li')).toBe('https://linkedin.com/in/saeed');
    expect(summary.filledNothing).toBe(false);
    expect(summary.nameSplit).toBeNull(); // single full-name field, no split
  });

  it('fills via Tier-1 autocomplete tokens (email/url/city) and given/family split', () => {
    setForm(`
      <input id="e" autocomplete="email" />
      <input id="w" autocomplete="url" />
      <input id="city" autocomplete="address-level2" />
      <input id="gn" autocomplete="given-name" />
      <input id="fam" autocomplete="family-name" />
    `);

    const summary = planAndFill(document, PROFILE);

    expect(val('e')).toBe('saeed@example.com');
    expect(val('w')).toBe('https://saeed.dev');
    expect(val('city')).toBe('Amsterdam, Netherlands');
    expect(val('gn')).toBe('Saeed');
    expect(val('fam')).toBe('Kolivand');
    // given/family came from splitting the full name → flagged.
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
  });

  it('dispatches an input event so framework-controlled inputs notice', () => {
    setForm(`<label for="email">Email</label><input id="email" type="email" />`);
    let fired = false;
    document.getElementById('email')!.addEventListener('input', () => {
      fired = true;
    });

    planAndFill(document, PROFILE);
    expect(fired).toBe(true);
  });

  it('counts multiple fields that receive the same value', () => {
    setForm(`
      <input id="e1" autocomplete="email" />
      <label for="e2">Email address</label><input id="e2" type="email" />
    `);

    const summary = planAndFill(document, PROFILE);
    const email = summary.filled.find((f) => f.key === 'email');
    expect(email?.count).toBe(2);
    expect(val('e1')).toBe('saeed@example.com');
    expect(val('e2')).toBe('saeed@example.com');
  });
});

describe('planAndFill – skips ambiguous / sensitive / hidden / filled', () => {
  it('never overwrites an already-filled field', () => {
    setForm(
      `<label for="email">Email</label><input id="email" type="email" value="keep@me.com" />`
    );
    planAndFill(document, PROFILE);
    expect(val('email')).toBe('keep@me.com');
  });

  it('skips password, hidden, and search inputs', () => {
    setForm(`
      <input id="pw" type="password" autocomplete="email" />
      <input id="hid" type="hidden" autocomplete="email" />
      <label for="s">Search jobs</label><input id="s" type="search" />
    `);
    planAndFill(document, PROFILE);
    expect(val('pw')).toBe('');
    expect(val('hid')).toBe('');
    expect(val('s')).toBe('');
  });

  it('skips a credit-card autocomplete token', () => {
    setForm(`<input id="cc" autocomplete="cc-number" />`);
    planAndFill(document, PROFILE);
    expect(val('cc')).toBe('');
  });

  it('skips ambiguous labels (referrer, company, confirm, emergency, manager)', () => {
    setForm(`
      <label for="ref">Referrer email</label><input id="ref" type="email" />
      <label for="co">Company website</label><input id="co" type="url" />
      <label for="ce">Confirm email</label><input id="ce" type="email" />
      <label for="em">Emergency phone</label><input id="em" type="tel" />
      <label for="mgr">Manager name</label><input id="mgr" type="text" />
    `);
    planAndFill(document, PROFILE);
    expect(val('ref')).toBe('');
    expect(val('co')).toBe('');
    expect(val('ce')).toBe('');
    expect(val('em')).toBe('');
    expect(val('mgr')).toBe('');
  });

  it('never touches a textarea (cover letter / why this role)', () => {
    document.body.innerHTML = `
      <form>
        <label for="cl">Why this role</label>
        <textarea id="cl" autocomplete="email"></textarea>
      </form>`;
    planAndFill(document, PROFILE);
    expect((document.getElementById('cl') as HTMLTextAreaElement).value).toBe('');
  });

  it('skips a field hidden by an ancestor display:none', () => {
    setForm(`<div style="display:none"><input id="dn" autocomplete="email" /></div>`);
    planAndFill(document, PROFILE);
    expect(val('dn')).toBe('');
  });

  it('skips a field hidden by an ancestor CSS CLASS (honeypot), not just inline style', () => {
    // Real anti-bot honeypots (Greenhouse/Lever/Workday) hide the trap field via an
    // external-stylesheet / <style> utility class, never an inline style="display:none" —
    // an inline-only check would miss this and fill (and thus flag) the honeypot.
    const style = document.createElement('style');
    style.setAttribute('data-ajh-test', '');
    style.textContent = '.ajh-visually-hidden { display: none; }';
    document.head.appendChild(style);

    setForm(`<div class="ajh-visually-hidden"><input id="hp" autocomplete="email" /></div>`);
    planAndFill(document, PROFILE);
    expect(val('hp')).toBe('');
  });

  it('skips a field hidden by an ancestor with opacity:0 (honeypot), but still fills a normal sibling', () => {
    const style = document.createElement('style');
    style.setAttribute('data-ajh-test', '');
    style.textContent = '.ajh-opacity-trap { opacity: 0; }';
    document.head.appendChild(style);

    setForm(`
      <div class="ajh-opacity-trap"><input id="op" autocomplete="email" /></div>
      <label for="normal">Email</label><input id="normal" type="email" />
    `);
    planAndFill(document, PROFILE);
    expect(val('op')).toBe('');
    // Guard against false positives: a normal visible field must still fill.
    expect(val('normal')).toBe('saeed@example.com');
  });

  it('skips a field shoved off-screen via position:absolute + left:-9999px (honeypot), but still fills a normal sibling', () => {
    setForm(`
      <div style="position:absolute; left:-9999px;"><input id="off" autocomplete="email" /></div>
      <label for="normal2">Email</label><input id="normal2" type="email" />
    `);
    planAndFill(document, PROFILE);
    expect(val('off')).toBe('');
    // Guard against false positives: a normal visible field must still fill.
    expect(val('normal2')).toBe('saeed@example.com');
  });

  it('under-fills: a bare "Website" is skipped, but "Portfolio" is filled', () => {
    setForm(`
      <label for="w1">Website</label><input id="w1" type="url" />
      <label for="w2">Portfolio URL</label><input id="w2" type="url" />
    `);
    planAndFill(document, PROFILE);
    expect(val('w1')).toBe(''); // ambiguous bare "Website" → under-fill
    expect(val('w2')).toBe('https://saeed.dev');
  });

  it('does not mis-fill education "Name" fields (School/University/Degree) with the full name', () => {
    setForm(`
      <label for="school">School Name</label><input id="school" type="text" />
      <label for="uni">University Name</label><input id="uni" type="text" />
      <label for="deg">Degree Name</label><input id="deg" type="text" />
      <label for="course">Course Name</label><input id="course" type="text" />
    `);
    planAndFill(document, PROFILE);
    expect(val('school')).toBe('');
    expect(val('uni')).toBe('');
    expect(val('deg')).toBe('');
    expect(val('course')).toBe('');
  });

  it('skips sensitive PII fields (SSN, passport, date of birth) even though the matcher never targets them', () => {
    setForm(`
      <label for="ssn">SSN</label><input id="ssn" type="text" autocomplete="email" />
      <label for="pp">Passport number</label><input id="pp" type="text" autocomplete="email" />
      <label for="dob">Date of birth</label><input id="dob" type="text" autocomplete="email" />
    `);
    planAndFill(document, PROFILE);
    expect(val('ssn')).toBe('');
    expect(val('pp')).toBe('');
    expect(val('dob')).toBe('');
  });

  it('skips a "Driver\'s license number" field even though the matcher never targets it', () => {
    setForm(
      `<label for="dl">Driver's license number</label><input id="dl" type="text" autocomplete="email" />`
    );
    planAndFill(document, PROFILE);
    expect(val('dl')).toBe('');
  });

  it('does not map structured address sub-parts (street) from a single location string', () => {
    setForm(`<input id="street" autocomplete="street-address" />`);
    planAndFill(document, PROFILE);
    expect(val('street')).toBe('');
  });

  it('leaves a matched field empty when the profile has no value for it', () => {
    setForm(`<label for="gh">GitHub</label><input id="gh" type="url" />`);
    planAndFill(document, { email: 'x@y.z' }); // no github in profile
    expect(val('gh')).toBe('');
  });
});

describe('planAndFill – name-split flag', () => {
  it('flags the split when separate first/last fields are filled', () => {
    setForm(`
      <label for="first">First name</label><input id="first" />
      <label for="last">Last name</label><input id="last" />
    `);

    const summary = planAndFill(document, PROFILE);
    expect(val('first')).toBe('Saeed');
    expect(val('last')).toBe('Kolivand');
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
    expect(summary.filled.map((f) => f.key).sort()).toEqual(['firstName', 'lastName']);
  });
});

describe('planAndFill – filled-nothing', () => {
  it('reports filledNothing when no field matches', () => {
    setForm(`
      <input id="pw" type="password" />
      <label for="s">Search</label><input id="s" type="search" />
      <textarea id="ta"></textarea>
    `);
    const summary = planAndFill(document, PROFILE);
    expect(summary.filledNothing).toBe(true);
    expect(summary.filled).toHaveLength(0);
  });
});

describe('planAndFill – Tier-2 extra-link matching', () => {
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

  it('fills a field whose label unambiguously matches one extra link', () => {
    setForm(`<label for="p">Portfolio</label><input id="p" type="url" />`);
    const summary = planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('p')).toBe('https://saeed.dev/work');
    expect(summary.filled).toContainEqual({
      key: 'extraLink:Portfolio',
      label: 'Portfolio',
      count: 1,
    });
  });

  it('matches case/diacritic-insensitively, as whole-word tokens (not a substring)', () => {
    setForm(`<label for="so">Stäck Overflöw profile</label><input id="so" type="url" />`);
    const profile: AutofillProfile = {
      extraLinks: [{ label: 'Stack Overflow', url: 'https://stackoverflow.com/users/1' }],
    };
    planAndFill(document, profile);
    expect(val('so')).toBe('https://stackoverflow.com/users/1');
  });

  it('requires a whole-word token match, not a coincidental substring (e.g. "Dribbble" must not match "Dribbblers")', () => {
    setForm(`<label for="d">Dribbblers only</label><input id="d" type="url" />`);
    const profile: AutofillProfile = {
      extraLinks: [{ label: 'Dribbble', url: 'https://dribbble.com/saeed' }],
    };
    planAndFill(document, profile);
    expect(val('d')).toBe('');
  });

  it('skips (ambiguous) a field whose signal matches MULTIPLE extra links, and flags it in the summary', () => {
    setForm(`<label for="both">Portfolio Dribbble</label><input id="both" type="url" />`);
    const summary = planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('both')).toBe('');
    expect(summary.skippedAmbiguous).toBe(1);
    expect(summary.filled).toHaveLength(0);
  });

  it('does NOT match a bare "Website" field label to an extra link literally labelled "Website"', () => {
    setForm(`<label for="w">Website</label><input id="w" type="url" />`);
    const profile: AutofillProfile = {
      extraLinks: [{ label: 'Website', url: 'https://saeed.dev/secondary' }],
    };
    const summary = planAndFill(document, profile);
    expect(val('w')).toBe('');
    expect(summary.skippedAmbiguous ?? 0).toBe(0);
    expect(summary.filledNothing).toBe(true);
  });

  it('never overwrites an already-filled field, even when its label matches a link', () => {
    setForm(`<label for="p">Portfolio</label><input id="p" type="url" value="https://keep.me" />`);
    planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('p')).toBe('https://keep.me');
  });

  it('never fills a hidden (honeypot) field even when its label matches a link', () => {
    setForm(
      `<div style="display:none"><label for="hp">Portfolio</label><input id="hp" type="url" /></div>`
    );
    planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('hp')).toBe('');
  });

  it('leaves a field with no matching link untouched', () => {
    setForm(`<label for="cl">Cover letter link</label><input id="cl" type="url" />`);
    const summary = planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('cl')).toBe('');
    expect(summary.filledNothing).toBe(true);
  });

  it('a field filled by a named key WITH a value is never additionally reconsidered against extraLinks', () => {
    setForm(`<label for="li">LinkedIn profile</label><input id="li" type="url" />`);
    const profile: AutofillProfile = {
      linkedin: 'https://linkedin.com/in/saeed',
      extraLinks: [{ label: 'LinkedIn Extra', url: 'https://example.com/other' }],
    };
    planAndFill(document, profile);
    expect(val('li')).toBe('https://linkedin.com/in/saeed');
  });

  it('does NOT fall through to the extra-link matcher for a non-website named key with an empty profile value (only `website` falls through)', () => {
    setForm(`<label for="li">LinkedIn</label><input id="li" type="url" />`);
    const profile: AutofillProfile = {
      extraLinks: [{ label: 'LinkedIn', url: 'https://linkedin.com/in/other' }],
    };
    const summary = planAndFill(document, profile);
    expect(val('li')).toBe(''); // named `linkedin` key claims it; no fallthrough
    expect(summary.filledNothing).toBe(true);
  });

  it('never fills an email/tel-typed field via the extra-link matcher (a URL is syntactically invalid there)', () => {
    setForm(`<label for="pe">Portfolio</label><input id="pe" type="email" />`);
    const summary = planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('pe')).toBe('');
    expect(summary.filledNothing).toBe(true);
  });

  it('never matches a link labelled a bare "Profile" (GENERIC_LINK_LABELS)', () => {
    setForm(`<label for="prof">Profile</label><input id="prof" type="url" />`);
    const profile: AutofillProfile = {
      extraLinks: [{ label: 'Profile', url: 'https://example.com/profile' }],
    };
    const summary = planAndFill(document, profile);
    expect(val('prof')).toBe('');
    expect(summary.filledNothing).toBe(true);
  });

  it('is a no-op when the profile has no extraLinks (absence tolerated)', () => {
    // "Dribbble" matches no existing Tier 1/2 named-key heuristic, so this
    // field is left untouched purely by the `links.length === 0` short-circuit.
    setForm(`<label for="d">Dribbble</label><input id="d" type="url" />`);
    const summary = planAndFill(document, PROFILE);
    expect(val('d')).toBe('');
    expect(summary.skippedAmbiguous ?? 0).toBe(0);
  });

  it('token-normalizes the generic-label denylist against punctuation/hyphen variants', () => {
    // Matching is token-based, so a bare exact-string check on the denylist
    // (e.g. "website!" not literally in the set) would let these slip through
    // while still token-matching the plain field label.
    const cases: Array<[linkLabel: string, fieldLabel: string]> = [
      ['Website!', 'Website'],
      ['Web-Site', 'Web Site'],
      ['Personal-Site', 'Personal Site'],
    ];
    for (const [linkLabel, fieldLabel] of cases) {
      setForm(`<label for="f">${fieldLabel}</label><input id="f" type="url" />`);
      const summary = planAndFill(document, {
        extraLinks: [{ label: linkLabel, url: 'https://example.com/x' }],
      });
      expect(val('f')).toBe('');
      expect(summary.filledNothing).toBe(true);
    }
  });

  it('token-normalizes the generic-label denylist independent of word order (e.g. "Site Web" vs "Website")', () => {
    // The denylist comparison must be order-insensitive since the field
    // matcher itself is (tokens.every) — otherwise "Site Web" would bypass
    // the denylisted "web site" while still token-matching a "Website" field.
    setForm(`<label for="f">Website</label><input id="f" type="url" />`);
    const summary = planAndFill(document, {
      extraLinks: [{ label: 'Site Web', url: 'https://example.com/x' }],
    });
    expect(val('f')).toBe('');
    expect(summary.filledNothing).toBe(true);
  });

  it('fills BOTH fields when two fields share one label and one matching link exists', () => {
    setForm(`
      <label for="p1">Portfolio</label><input id="p1" type="url" />
      <label for="p2">Portfolio</label><input id="p2" type="url" />
    `);
    const summary = planAndFill(document, PROFILE_WITH_LINKS);
    expect(val('p1')).toBe('https://saeed.dev/work');
    expect(val('p2')).toBe('https://saeed.dev/work');
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
    setForm(`<label for="up">Uberprofil</label><input id="up" type="url" />`);
    const summary = planAndFill(document, {
      extraLinks: [{ label: 'Überprofil', url: 'https://example.com/uber' }],
    });
    expect(val('up')).toBe('https://example.com/uber');
    expect(summary.filled).toContainEqual({
      key: 'extraLink:Überprofil',
      label: 'Überprofil',
      count: 1,
    });
  });
});

describe('renderSummaryOverlay', () => {
  it('renders a dismissable overlay listing the filled fields', () => {
    const summary = {
      filled: [
        { key: 'email', label: 'Email', count: 2 },
        { key: 'firstName', label: 'First name', count: 1 },
      ],
      nameSplit: { first: 'Saeed', last: 'Kolivand' },
      filledNothing: false,
    };
    renderSummaryOverlay(document, summary);

    const overlay = document.getElementById('ajh-autofill-overlay');
    expect(overlay).not.toBeNull();
    expect(overlay!.textContent).toContain('Email → 2 fields');
    expect(overlay!.textContent).toContain('First name → 1 field');
    expect(overlay!.textContent).toContain('Name split (guess)');
    expect(overlay!.textContent).toContain('Saeed');

    // Dismiss removes it.
    const dismiss = overlay!.querySelector('button')!;
    dismiss.dispatchEvent(new Event('click', { bubbles: true }));
    expect(document.getElementById('ajh-autofill-overlay')).toBeNull();
  });

  it('renders the "nothing matched" message so a no-op does not look broken', () => {
    renderSummaryOverlay(document, { filled: [], nameSplit: null, filledNothing: true });
    const overlay = document.getElementById('ajh-autofill-overlay');
    expect(overlay!.textContent).toContain('No matchable fields found');
  });

  it('replaces a prior overlay instead of stacking', () => {
    renderSummaryOverlay(document, { filled: [], nameSplit: null, filledNothing: true });
    renderSummaryOverlay(document, { filled: [], nameSplit: null, filledNothing: true });
    expect(document.querySelectorAll('#ajh-autofill-overlay')).toHaveLength(1);
  });

  it('notes skipped-ambiguous extra-link fields alongside a successful fill', () => {
    renderSummaryOverlay(document, {
      filled: [{ key: 'extraLink:Portfolio', label: 'Portfolio', count: 1 }],
      nameSplit: null,
      filledNothing: false,
      skippedAmbiguous: 2,
    });
    const overlay = document.getElementById('ajh-autofill-overlay');
    expect(overlay!.textContent).toContain('Portfolio → 1 field');
    expect(overlay!.textContent).toContain('2 fields skipped');
  });

  it('omits the skipped-ambiguous note when there is nothing to report', () => {
    renderSummaryOverlay(document, {
      filled: [{ key: 'email', label: 'Email', count: 1 }],
      nameSplit: null,
      filledNothing: false,
      skippedAmbiguous: 0,
    });
    const overlay = document.getElementById('ajh-autofill-overlay');
    expect(overlay!.textContent).not.toContain('skipped');
  });

  it('suppresses the "no matchable fields" line when fields were skipped as ambiguous instead', () => {
    // filledNothing + skippedAmbiguous both true reads as contradictory
    // ("no matchable fields" + "N fields skipped") — the skipped-note alone
    // already explains the outcome.
    renderSummaryOverlay(document, {
      filled: [],
      nameSplit: null,
      filledNothing: true,
      skippedAmbiguous: 1,
    });
    const overlay = document.getElementById('ajh-autofill-overlay');
    expect(overlay!.textContent).not.toContain('No matchable fields found');
    expect(overlay!.textContent).toContain('1 field skipped');
  });
});

describe('runAutofill', () => {
  it('fills the document and injects the summary overlay, returning the summary', () => {
    setForm(`<label for="email">Email</label><input id="email" type="email" />`);
    const summary = runAutofill(PROFILE);

    expect(val('email')).toBe('saeed@example.com');
    expect(document.getElementById('ajh-autofill-overlay')).not.toBeNull();
    expect(summary.filledNothing).toBe(false);
    expect(summary.filled.map((f) => f.key)).toContain('email');
  });
});

describe('AUTOFILL_GLOBAL', () => {
  it('is pinned — background.ts hardcodes the same literal (kept in lockstep)', () => {
    // background.ts intentionally duplicates this literal (it cannot runtime-import
    // autofill.ts, or fill.js would gain an ES import and break classic injection).
    // If this value changes, update the local const in background.ts too.
    expect(AUTOFILL_GLOBAL).toBe('__ajhRunAutofill');
  });
});

describe("hasAutofillableFields — the popup fields-probe's WIDER signal (Form group gating)", () => {
  it('returns false for a page with no form fields at all (a plain job listing)', () => {
    document.body.innerHTML = `<p>Senior Rust Engineer at Acme Corp.</p>`;
    expect(hasAutofillableFields(document)).toBe(false);
  });

  it("returns true for an IDENTITY-ONLY form (name/email/phone) — the exact case answers-capture's narrower signal misses, since it excludes identity fields by design", () => {
    setForm(`
      <label for="name">Full name</label><input id="name" type="text" value="" />
      <label for="email">Email</label><input id="email" type="email" value="" />
      <label for="phone">Phone</label><input id="phone" type="tel" value="" />
    `);
    expect(hasAutofillableFields(document)).toBe(true);
  });

  it('returns true when only ONE identity field is present', () => {
    setForm(`<label for="email">Email</label><input id="email" type="email" value="" />`);
    expect(hasAutofillableFields(document)).toBe(true);
  });

  it('returns false when every input is non-identity/ambiguous/hidden (nothing autofill would ever touch)', () => {
    setForm(`
      <label for="q1">Why this role?</label><input id="q1" type="text" value="" />
      <label for="pw">Password</label><input id="pw" type="password" value="" />
      <label for="h">Honeypot email</label><input id="h" type="email" value="" style="display:none" />
    `);
    expect(hasAutofillableFields(document)).toBe(false);
  });

  it('returns false for an already-FILLED identity field (autofill never overwrites — nothing left for Fill to do)', () => {
    setForm(
      `<label for="email">Email</label><input id="email" type="email" value="already@example.com" />`
    );
    expect(hasAutofillableFields(document)).toBe(false);
  });
});
