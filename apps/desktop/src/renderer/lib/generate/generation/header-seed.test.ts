import { describe, expect, it } from 'vitest';

import { seedHeaderFromProfile } from './generation';

describe('seedHeaderFromProfile — header-boundary edge cases (security review)', () => {
  const PROFILE = { fullName: 'Jordan Lee', email: 'jordan@profile.example.com' };
  const CONTACT_LINE = 'Berlin | jordan@profile.example.com';

  it('replaces the conformant name/contact layout (baseline)', () => {
    const text = 'Model Name\nmodel@old.example.com | +1 555 0000\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('Jordan Lee\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
  });

  // Security re-review (CRITICAL): the header is, by definition, the first
  // blank-line-delimited block — the scan/splice never looks past it, so a
  // contact line separated from the name by its OWN blank line falls outside
  // that block and is neither found nor removed. The seeder still inserts
  // the profile's line right after the name, so the profile's contact info
  // does get seeded; the model's own (now out-of-block) line survives
  // untouched alongside it. A duplicate line is the accepted, non-destructive
  // trade-off for the structural safety bound below — it can never delete
  // real content the way an unbounded scan could.
  it('does not remove a contact line separated from the name by its own blank line — inserts instead', () => {
    const text = 'Model Name\n\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      'Jordan Lee\nBerlin | jordan@profile.example.com\n\nmodel@old.example.com\n\nSUMMARY\nSome text.'
    );
  });

  // Security re-review (HIGH, round 2): this function never removes a line —
  // only the email-bearing match is replaced (positive signal, not
  // position); the phone-only match survives as a duplicate rather than
  // being spliced out. Deleting it was the actual defect (see the job-title
  // repro below); a surviving duplicate is the accepted trade.
  it('replaces the email-bearing match; a second pre-section contact line (phone on its own line) survives as a duplicate', () => {
    const text = 'Model Name\nmodel@old.example.com\n+1 555 0000\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      'Jordan Lee\nBerlin | jordan@profile.example.com\n+1 555 0000\n\nSUMMARY\nSome text.'
    );
  });

  it('replaces the email-bearing match; a second pre-section contact line (URLs on their own line) survives as a duplicate', () => {
    const text =
      'Model Name\nmodel@old.example.com\n[Portfolio](https://old.example.dev) | [GitHub](https://github.com/old)\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      'Jordan Lee\nBerlin | jordan@profile.example.com\n[Portfolio](https://old.example.dev) | [GitHub](https://github.com/old)\n\nSUMMARY\nSome text.'
    );
  });

  it('finds an ALL-CAPS contact line instead of mistaking it for a section heading', () => {
    const text = 'Model Name\nBERLIN | MODEL@OLD.EXAMPLE.COM\n\nSUMMARY\nSome text.';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe('Jordan Lee\nBerlin | jordan@profile.example.com\n\nSUMMARY\nSome text.');
    expect(out).not.toContain('MODEL@OLD.EXAMPLE.COM');
  });

  it('is idempotent on a phone+link-only contact line (no email) across a second seeding pass', () => {
    // A profile with only a phone and one link (no email) is exactly the shape
    // that used to be missed pre-parity-fix: one pipe, no `@`.
    const profile = { fullName: 'Jordan Lee', phone: '+49 30 0000000', linkedin: 'x' };
    const contactLine = '+49 30 0000000 | [LinkedIn](https://linkedin.com/in/jordan)';
    const text = 'Model Name\nmodel@old.example.com\n\nSUMMARY\nSome text.';
    const once = seedHeaderFromProfile(text, profile, contactLine);
    const twice = seedHeaderFromProfile(once, profile, contactLine);
    expect(twice).toBe(once);
    // Assert on the whole seeded line, not a bare host substring: counting
    // `includes('linkedin.com')` reads as URL-host sanitization to CodeQL
    // (js/incomplete-url-substring-sanitization) and is weaker anyway — it
    // would pass if the line were mangled as long as the host survived.
    expect(once.split('\n').filter((l) => l === contactLine).length).toBe(1);
  });

  // Security re-review (HIGH-1): the exact repro — the prompt mandates "Line
  // 2: Job title", and an ALL-CAPS title used to stop the scan before it ever
  // reached the real contact line below.
  it('does not mistake an ALL-CAPS job title for a header boundary', () => {
    const text =
      'Jane Doe\nSENIOR SOFTWARE ENGINEER\njane@example.com | +49 30 1234567\n\nEXPERIENCE\nAcme Corp';
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      'Jordan Lee\nSENIOR SOFTWARE ENGINEER\nBerlin | jordan@profile.example.com\n\nEXPERIENCE\nAcme Corp'
    );
    // No duplicate contact line, and the title survives untouched.
    expect(out.split('\n').filter((l) => l.includes('@')).length).toBe(1);
  });

  // Security re-review (MEDIUM): a combined name+contact line 0 with a
  // fullName-less profile — Rust's idx==0 rule classifies that line as
  // Contact, not Name, so it must be in scope for the scan too, not silently
  // skipped. But (round 4) index 0 is never itself the replacement target —
  // overwriting it here would erase "Jane Doe" (there is no separate name
  // line to preserve it) even though the profile has nothing to offer for
  // the name. Insert instead: both survive.
  it('preserves the name when line 0 combines name+contact and the profile has no fullName to write over it', () => {
    const profile = { phone: '+49 30 0000000' };
    const contactLine = '+49 30 0000000';
    const text = 'Jane Doe | jane@old.example.com\n\nEXPERIENCE\nAcme Corp';
    const out = seedHeaderFromProfile(text, profile, contactLine);
    expect(out).toBe('Jane Doe | jane@old.example.com\n+49 30 0000000\n\nEXPERIENCE\nAcme Corp');
  });

  // Security re-review (HIGH, round 2) — the actual required regression: the
  // prompt mandates "Line 2: Job title (plain text)", models routinely put
  // separators in it ("Senior Engineer | Cloud & AI | Berlin" → 2 pipes →
  // contact-shaped), and it sits BEFORE the real, `@`-bearing contact line in
  // the same header block. The old "replace matches[0], splice the rest"
  // policy deleted the title. Never-remove + pick-by-positive-signal (prefer
  // `@`) keeps it, verbatim, and still seeds the profile's contact line onto
  // the correct target.
  it('does not delete a separator-bearing job title sitting before the real contact line', () => {
    const text = [
      'Jane Doe',
      'Senior Engineer | Cloud & AI | Berlin',
      'Madrid, Spain | jane@example.com | +34 600 000 000 | LinkedIn | GitHub',
      '',
      'PERFIL',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      [
        'Jordan Lee',
        'Senior Engineer | Cloud & AI | Berlin',
        'Berlin | jordan@profile.example.com',
        '',
        'PERFIL',
      ].join('\n')
    );
  });

  // The other reachable path to the same class of loss: a heading
  // COMPANY_KEYWORDS vetoes out of isAllCapsSectionHeading ("IT" is a
  // keyword) and that isn't in SECTION_NAMES either ("IT SKILLS" isn't a
  // known name) — recognized as a boundary by neither predicate, and with no
  // blank line anywhere the whole document is one block. Never-remove keeps
  // every line under it, whatever else in the block also happens to look
  // contact-shaped (2+ pipes).
  it('never deletes lines under a heading COMPANY_KEYWORDS vetoes and SECTION_NAMES does not know ("IT SKILLS")', () => {
    const text = [
      'Jane Doe',
      'jane@example.com | +1 555 0000',
      'IT SKILLS',
      'Python | JavaScript | Go',
      'React | Node | Docker',
      'AWS | GCP | Kubernetes',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toContain('IT SKILLS');
    expect(out).toContain('Python | JavaScript | Go');
    expect(out).toContain('React | Node | Docker');
    expect(out).toContain('AWS | GCP | Kubernetes');
    // Exactly one contact line remains — the seeded one.
    expect(out.split('\n').filter((l) => l.includes('@')).length).toBe(1);
  });

  // Security re-review (CRITICAL): `packages/prompts/src/locale/index.ts`'s
  // `CONVENTIONS` ships résumé headers for es/it/nl/pt too, and the résumé
  // prompt mandates them ALL-CAPS. Before the ALL-CAPS shape rule was
  // restored (fixture-gated this time), none of these headings stopped the
  // scan, which then matched body prose ("portfolio de productos SaaS") and a
  // job-entry date range ("(2021 - 2023)", phone-shaped) as false contact
  // lines and DELETED them. No blank line before the heading here — that's
  // what actually exercises heading recognition rather than the separate
  // structural (first-blank-line) bound.
  it('does not delete Spanish résumé body content — the exact CRITICAL repro', () => {
    const text = [
      'Jane Doe',
      'jane@example.com | +34 600 000 000',
      'EXPERIENCIA PROFESIONAL',
      'Ingeniero con experiencia en portfolio de productos SaaS.',
      'Ingeniero Senior, Acme Corp (2021 - 2023)',
      '',
      'HABILIDADES',
      'Lenguajes | Frameworks | Herramientas',
      '',
      'PROYECTOS',
      'Mi Proyecto — [Repo](https://github.com/jane/proj)',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      [
        'Jordan Lee',
        'Berlin | jordan@profile.example.com',
        'EXPERIENCIA PROFESIONAL',
        'Ingeniero con experiencia en portfolio de productos SaaS.',
        'Ingeniero Senior, Acme Corp (2021 - 2023)',
        '',
        'HABILIDADES',
        'Lenguajes | Frameworks | Herramientas',
        '',
        'PROYECTOS',
        'Mi Proyecto — [Repo](https://github.com/jane/proj)',
      ].join('\n')
    );
  });

  it.each([
    ['it', 'ESPERIENZA PROFESSIONALE', 'Ingegnere con esperienza in portfolio di prodotti SaaS.'],
    ['nl', 'WERKERVARING', 'Ingenieur met ervaring in portfolio van SaaS-producten.'],
    ['pt', 'EXPERIÊNCIA PROFISSIONAL', 'Engenheiro com experiência em portfolio de produtos SaaS.'],
  ])('does not delete %s résumé body content (heading %s)', (_locale, heading, bodyLine) => {
    const text = [
      'Jane Doe',
      'jane@example.com | +1 555 0000',
      heading,
      bodyLine,
      'Senior Engineer, Acme Corp (2021 - 2023)',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toContain(bodyLine);
    expect(out).toContain('Senior Engineer, Acme Corp (2021 - 2023)');
  });

  // An English heading that's a real, common résumé section title but not a
  // VERBATIM match in SECTION_NAMES (which has "work experience", not
  // "professional experience") — the ALL-CAPS shape rule, not the known-name
  // list, is what has to catch this one.
  it('recognizes "PROFESSIONAL EXPERIENCE" via the ALL-CAPS shape rule, not literally in SECTION_NAMES', () => {
    const text = [
      'Jane Doe',
      'jane@example.com | +1 555 0000',
      'PROFESSIONAL EXPERIENCE',
      'Senior Engineer, Acme Corp (2021 - 2023)',
      '- Led a team of five engineers',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toBe(
      [
        'Jordan Lee',
        'Berlin | jordan@profile.example.com',
        'PROFESSIONAL EXPERIENCE',
        'Senior Engineer, Acme Corp (2021 - 2023)',
        '- Led a team of five engineers',
      ].join('\n')
    );
  });

  // The structural backstop itself: even a heading `looksLikeHeaderBoundary`
  // can't recognize at all must never cause data loss — the scan is bounded
  // to the first blank-line-delimited block regardless.
  it('never deletes content past the first blank line, even for a wholly unrecognized heading', () => {
    const text = [
      'Jane Doe',
      'jane@example.com | +1 555 0000',
      '',
      '★ A CREATIVE HEADING NO PREDICATE RECOGNIZES ★',
      'A job entry with a phone-shaped date (2021 - 2023) that must survive.',
      'Skills | Frameworks | Tools',
    ].join('\n');
    const out = seedHeaderFromProfile(text, PROFILE, CONTACT_LINE);
    expect(out).toContain('A job entry with a phone-shaped date (2021 - 2023) that must survive.');
    expect(out).toContain('Skills | Frameworks | Tools');
    expect(out).toContain('★ A CREATIVE HEADING NO PREDICATE RECOGNIZES ★');
  });
});
