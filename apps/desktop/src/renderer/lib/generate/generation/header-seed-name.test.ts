import { describe, expect, it } from 'vitest';

import { seedHeaderFromProfile } from './generation';

describe('seedHeaderFromProfile — fullName handling and name reconciliation (security review)', () => {
  const PROFILE = { fullName: 'Jordan Lee', email: 'jordan@profile.example.com' };
  const CONTACT_LINE = 'Berlin | jordan@profile.example.com';

  // Security re-review (MEDIUM): `fullName` is spliced into the seeded text
  // directly, not via `contactLine` (which is already sanitized — it's built
  // by Rust's `header_markdown`). A raw newline in `fullName` must not
  // fabricate physical lines Rust's parser could reclassify as a section.
  it('sanitizes a fullName containing control characters before splicing it into line 0', () => {
    const profile = {
      fullName: 'Jordan Lee\nAWARDS\nNobel Prize in Physics, 2024',
      email: 'jordan@profile.example.com',
    };
    const text = 'Model Name\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, profile, CONTACT_LINE);
    const lines = out.split('\n');
    expect(lines[0]).toBe('Jordan LeeAWARDSNobel Prize in Physics, 2024');
    expect(out).not.toContain('\nAWARDS\n');
  });

  // MEDIUM (security re-review): sanitizeHeaderName's cap iterates Unicode
  // CODE POINTS (`[...name]`), not UTF-16 units (`.slice`) — an astral
  // character (outside the BMP, a surrogate PAIR in UTF-16) straddling the
  // 200 boundary would otherwise split into a lone, invalid surrogate. These
  // two cases pin the boundary from both sides: just inside the cap, the
  // whole character survives; just past it, the whole character is excluded
  // — never half of one either way.
  it('caps fullName at 200 CODE POINTS, not UTF-16 units — an astral character just inside the cap survives whole', () => {
    const fullName = 'A'.repeat(199) + '😀' + 'BBBB';
    const out = seedHeaderFromProfile('Some AI Written Name\n\nSUMMARY\nBody.', { fullName }, '');
    const name = out.split('\n')[0] ?? '';
    expect([...name]).toHaveLength(200);
    expect(name).toBe(`${'A'.repeat(199)}😀`);
    expect(/[\uD800-\uDBFF]$/.test(name)).toBe(false); // no lone surrogate at the cut
  });

  it('caps fullName at 200 CODE POINTS — an astral character just past the cap is excluded whole, never split into a lone surrogate', () => {
    const fullName = `${'A'.repeat(200)}😀`;
    const out = seedHeaderFromProfile('Some AI Written Name\n\nSUMMARY\nBody.', { fullName }, '');
    const name = out.split('\n')[0] ?? '';
    expect([...name]).toHaveLength(200);
    expect(name).toBe('A'.repeat(200));
    expect(/[\uD800-\uDBFF]$/.test(name)).toBe(false);
  });

  // LOW (security re-review): sanitizeHeaderName strips `\p{Cf}` (Unicode
  // Format characters) too, not just `\p{Cc}` — a bidi override (U+202E)
  // left in place could visually REVERSE the surrounding rendered name.
  // Mirrors Rust's `is_format_char` in `contact_profile/header.rs`.
  it('strips a bidi override character from fullName', () => {
    // \u202E RIGHT-TO-LEFT OVERRIDE — a JS unicode escape, not a literal bidi
    // character in source (a literal one here would visually scramble this
    // file for anyone viewing it, the "Trojan Source" class of concern).
    const fullName = 'Berlin\u202EnilreB';
    const out = seedHeaderFromProfile('Some AI Written Name\n\nSUMMARY\nBody.', { fullName }, '');
    const name = out.split('\n')[0] ?? '';
    expect(name).toBe('BerlinnilreB');
    expect(name).not.toContain('\u202E');
  });

  // Security re-review (HIGH, round 4): `isContactProfileEffectivelyEmpty`
  // correctly excludes `fullName` when deciding whether there's a CONTACT
  // LINE to build, but it used to also gate the whole function's early
  // return — making the independent `if (fullName) …` branch below
  // unreachable for a fullName-only profile. There's no early return at all
  // now; the two guards (name / contact) are each self-sufficient.
  it('seeds the name from a fullName-only profile with no other contact fields', () => {
    const out = seedHeaderFromProfile(
      'Some AI Written Name\n\nSUMMARY\nBody.',
      { fullName: 'Jordan Lee' },
      ''
    );
    expect(out).toBe('Jordan Lee\n\nSUMMARY\nBody.');
  });

  // Security re-review (MEDIUM, round 4): a bare `@` (a job title like
  // "Software Engineer @ Acme") must not outrank a genuine email — that
  // inverts intent, overwriting the title and leaving the real, stale
  // contact line untouched.
  it('prefers a genuine email over a bare "@" when picking the replacement target', () => {
    const text = [
      'Jane Doe',
      'Software Engineer @ Acme',
      'Madrid | jane@example.com | +34 600 000 000',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      ['Jordan Lee', 'Software Engineer @ Acme', 'Berlin | jordan@profile.example.com'].join('\n')
    );
  });

  // Security re-review (MEDIUM, round 4): with no email/phone signal
  // anywhere, the no-signal fallback must pick the FIRST match, not the
  // last — the last is the one closest to the body, most likely to actually
  // BE body content a boundary-recognition miss let through.
  it('picks the FIRST no-signal match, not the last — a link-only header must not overwrite real body content', () => {
    const text = ['Jane Doe', 'Website | GitHub', 'AWS | GCP | Kubernetes'].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      ['Jordan Lee', 'Berlin | jordan@profile.example.com', 'AWS | GCP | Kubernetes'].join('\n')
    );
  });

  // Security re-review (MAJOR, round 6) — the exact repro: line 0 used to be
  // overwritten unconditionally whenever the profile has a fullName, with no
  // classification of what line 0 actually IS. A model that omits the name
  // line entirely (starts straight with a section heading) had that heading
  // destroyed and replaced with the name instead.
  it('does not destroy a section heading the model wrote on line 0 — inserts the name instead of overwriting it', () => {
    const out = seedHeaderFromProfile(
      'SUMMARY\nSenior engineer with …',
      { fullName: 'Jordan Lee' },
      ''
    );
    expect(out).toBe('Jordan Lee\nSUMMARY\nSenior engineer with …');
  });

  // Same guard, the other reachable shape: line 0 is already contact-shaped
  // (no separate name line at all) AND the profile has a fullName this time
  // (unlike the no-fullName case covered above) — the name must still be
  // inserted, never overwritten onto the contact line, and the guard must
  // compose cleanly with the contact-line scan that follows: the now-shifted
  // contact line at index 1 is still found and replaced normally.
  it('inserts the name ahead of a contact-shaped line 0 rather than overwriting it, when the profile has a fullName too', () => {
    const text = 'jane@old.example.com | +1 555 0000\n\nEXPERIENCE\nAcme Corp';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('Jordan Lee\nBerlin | jordan@profile.example.com\n\nEXPERIENCE\nAcme Corp');
  });

  // CodeRabbit (security re-review): follows from the line-0 guard above —
  // with no fullName to seed a name line, and line 0 already a section
  // heading (the model omitted the name entirely), the no-match insertion
  // used to hardcode index 1, which put the contact line INSIDE that
  // section, right under its own heading, rather than in the header block
  // above it. It must land BEFORE the heading instead.
  it('inserts the contact line before a first-line section heading, not inside the section under it', () => {
    const out = seedHeaderFromProfile(
      'SUMMARY\nSenior engineer with great experience.',
      { phone: '+49 30 0000000' },
      '+49 30 0000000'
    );
    expect(out).toBe('+49 30 0000000\nSUMMARY\nSenior engineer with great experience.');
  });

  // The exact repro this task closes (round 1): an ALL-CAPS own name at line
  // 0 passes `isAllCapsSectionHeading`, so `looksLikeHeaderBoundary` used to
  // read it as a section boundary and unshift the profile's name ABOVE it,
  // leaving "JORDAN LEE" behind as a fake body section carrying the title and
  // the model's original contact line — a fully duplicated header. It must
  // instead be reconciled IN PLACE: same line count, exactly one contact
  // line.
  it('reconciles an ALL-CAPS own name at line 0 in place instead of stacking a duplicate header', () => {
    const text = 'JORDAN LEE\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const beforeLineCount = text.split('\n').length;
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('Jordan Lee\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
    expect(out.split('\n')).toHaveLength(beforeLineCount);
  });

  // The other reachable repro (round 2): a leading blank line (PDF
  // extraction routinely emits one) used to get blindly replaced with the
  // name, leaving the model's real ALL-CAPS name line sitting untouched one
  // row down — where the contact scan then also broke, splicing in a SECOND
  // contact line. The name reconciliation search must find the name at index
  // 1 (not just index 0) and the real contact line below it must still be
  // found and replaced normally.
  it('reconciles an ALL-CAPS name at index 1 after a leading blank line, and still replaces the real contact line below it', () => {
    const text = '\nJORDAN LEE\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const beforeLineCount = text.split('\n').length;
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('\nJordan Lee\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
    expect(out.split('\n')).toHaveLength(beforeLineCount);
  });

  // Sibling-review finding 1 (HIGH): NFKD decomposes an accented character
  // into base letter + combining mark; the mark is category `Mn`, NOT
  // `\p{L}`/`\p{N}`, so collapsing it to a SPACE (an earlier version of
  // `nameKey` did) inserts a spurious word break — "François" keyed to
  // "franc ois", which never matches "FRANCOIS" (no diacritics at all, the
  // shape a document extracted without accent support produces). The
  // combining marks must be stripped outright before the punctuation
  // collapse, not turned into separators.
  it('reconciles an accented profile name against the same name written without diacritics (NFKD combining marks must be stripped, not spaced)', () => {
    const profile = { fullName: 'François Müller', email: 'contact@example.com' };
    const contactLine = 'Berlin | contact@example.com';
    const text = 'FRANCOIS MULLER\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, profile, contactLine);
    expect(out).toBe('François Müller\nBerlin | contact@example.com\n\nSUMMARY\nSome text.');
  });

  // Sibling-review finding 2 (HIGH): `sanitizeHeaderName` never changes case
  // — reconciling an ALL-CAPS `fullName` onto the ALL-CAPS document line
  // below a leading blank leaves that line ALL-CAPS, which re-trips
  // `looksLikeHeaderBoundary` right where the contact scan starts (i = 1),
  // breaking the scan before it reaches the real contact line and stacking a
  // second one via the insert branch instead of replacing the first.
  it('does not re-trip the contact scan when the reconciled name itself is ALL-CAPS', () => {
    const profile = { fullName: 'JORDAN LEE', email: 'jordan@profile.example.com' };
    const text = '\nJORDAN LEE\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, profile, CONTACT_LINE);
    expect(out).toBe('\nJORDAN LEE\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
  });

  // Sibling-review finding 3 (MEDIUM): `headerBlockEnd` returns
  // `lines.length` when the document has no blank line anywhere, so an
  // unbounded name search can match the profile's name recurring later in
  // the BODY (a sign-off line) and reconcile THAT occurrence instead of ever
  // seeding a name line at the top — strictly worse than the pre-existing
  // fallback, which always unshifts. `findNameLine` must stop at the first
  // `looksLikeHeaderBoundary` line (checking the name match FIRST, so an
  // ALL-CAPS name isn't blocked from matching itself).
  it('does not reconcile a coincidental body occurrence of the name — falls through to seeding one at the top instead', () => {
    const text = 'SUMMARY\nExperienced engineer.\nCertifications awarded\nJordan Lee';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      'Jordan Lee\nBerlin | jordan@profile.example.com\nSUMMARY\nExperienced engineer.\nCertifications awarded\nJordan Lee'
    );
  });

  // CodeRabbit (round 7): `headerBlockEnd` started its termination scan at a
  // hardcoded `i = 1`, which coincidentally still worked for exactly ONE
  // leading blank line (index 1 there IS the real name) but returns 1
  // immediately — the SECOND blank — for TWO OR MORE, making the name at
  // index 2 (and everything after it) unreachable to `findNameLine`. The
  // reconciliation search then finds nothing, falls through to the
  // unshift/replace fallback, and replaces line 0 (still blank) — leaving
  // the model's real ALL-CAPS name line sitting untouched below, exactly the
  // duplicated-header shape this whole file exists to close.
  it('reconciles the name and the contact line past TWO leading blank lines, not just one', () => {
    const text = '\n\nJORDAN LEE\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('\n\nJordan Lee\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
  });

  // CodeRabbit (round 8): the never-overwrite protection on the first
  // content line was hardcoded to index 0, which stopped covering the line
  // it protects once the header block itself started at `firstContentLine`
  // instead of index 0 (round 7). With ≥2 leading blanks and no `fullName`
  // (so `reconciledNameIndex` stays -1), a combined "Jane Doe |
  // jane@old.example.com" first content line now sits at index ≥ 2 — inside
  // the scan range, `isHeaderContactLine`-eligible, and (without this fix)
  // blind-overwritten with the profile's contact line, erasing "Jane Doe"
  // entirely since there's no separate name line to fall back to.
  it('does not overwrite a combined name+contact first content line behind two leading blank lines', () => {
    const profile = { phone: '+49 30 0000000' };
    const contactLine = '+49 30 0000000';
    const text = '\n\nJane Doe | jane@old.example.com\n\nEXPERIENCE\nAcme Corp';
    const out = seedHeaderFromProfile(text, profile, contactLine);
    expect(out).toBe(
      '\n\nJane Doe | jane@old.example.com\n+49 30 0000000\n\nEXPERIENCE\nAcme Corp'
    );
  });
});
