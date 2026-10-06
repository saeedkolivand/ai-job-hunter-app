import { describe, expect, it } from 'vitest';

import { injectLinksIntoGeneratedText } from '../index';

describe('injectLinksIntoGeneratedText', () => {
  it('replaces known labels in the contact line with markdown links', () => {
    const text = `John Doe\nSenior Engineer\nBerlin | john@example.com | LinkedIn | GitHub\n\nSUMMARY`;
    const out = injectLinksIntoGeneratedText(text, {
      LinkedIn: 'https://linkedin.com/in/jd',
      GitHub: 'https://github.com/jd',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/jd)');
    expect(out).toContain('[GitHub](https://github.com/jd)');
  });

  it('returns text unchanged when the link map is empty', () => {
    const text = 'Name\nRole\nCity | LinkedIn';
    expect(injectLinksIntoGeneratedText(text, {})).toBe(text);
  });

  it('does not touch section header lines', () => {
    const text = `WORK EXPERIENCE | something\nbody`;
    const out = injectLinksIntoGeneratedText(text, { LinkedIn: 'https://linkedin.com/in/x' });
    expect(out).toBe(text);
  });

  it('injects a Website link in the contact line', () => {
    const text = `Jane Doe\nDesigner\nBerlin | jane@example.com | Website | GitHub\n\nSUMMARY`;
    const out = injectLinksIntoGeneratedText(text, {
      Website: 'https://janedoe.dev',
      GitHub: 'https://github.com/jd',
    });
    expect(out).toContain('[Website](https://janedoe.dev)');
    expect(out).toContain('[GitHub](https://github.com/jd)');
  });

  it('finds the cover-letter contact line below the top (past the old 6-line window)', () => {
    // Regression: cover letters carry the contact line under a marker / name /
    // preamble, so the old fixed first-6-lines scan silently skipped it and
    // LinkedIn never got hyperlinked (Dribbble survived only as a bare URL).
    const coverLetter = [
      'COMPLETE COVER LETTER ###',
      '',
      'preamble one',
      'preamble two',
      'preamble three',
      'preamble four',
      'preamble five',
      'Lena Vos',
      'Amsterdam, Niederlande | lena.vos@example.com | +31 6 | LinkedIn | Dribbble',
      '',
      'Sehr geehrte Damen und Herren,',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(coverLetter, {
      LinkedIn: 'https://linkedin.com/in/lena-vos',
      Dribbble: 'https://dribbble.com/lenavos',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/lena-vos)');
    expect(out).toContain('[Dribbble](https://dribbble.com/lenavos)');
  });

  it('links only the email-bearing contact line, not body prose mentioning a platform', () => {
    const text = [
      'Lena Vos',
      'Amsterdam | lena.vos@example.com | LinkedIn',
      '',
      'I doubled our GitHub | community and shipped on LinkedIn weekly.',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(text, {
      LinkedIn: 'https://linkedin.com/in/lena-vos',
      GitHub: 'https://github.com/lenavos',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/lena-vos)');
    // The body sentence has a pipe but no email → left untouched.
    expect(out).toContain('I doubled our GitHub | community and shipped on LinkedIn weekly.');
    expect(out).not.toContain('[GitHub](https://github.com/lenavos)');
  });

  it('is idempotent — a second pass does not double-wrap links', () => {
    const text = 'Name\nCity | n@example.com | LinkedIn';
    const once = injectLinksIntoGeneratedText(text, { LinkedIn: 'https://linkedin.com/in/n' });
    const twice = injectLinksIntoGeneratedText(once, { LinkedIn: 'https://linkedin.com/in/n' });
    expect(twice).toBe(once);
  });

  it('injects body links onto their items anywhere in the body, not just the contact line (#18)', () => {
    const text = [
      'Jane Dev',
      'Researcher',
      'Berlin | jane@example.com | GitHub',
      '',
      'PROJECTS',
      '• orbit-sim — a relativistic orbit simulator',
      '',
      'PUBLICATIONS',
      '• Spin glasses in 2D, Phys Rev B (2021)',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(
      text,
      { GitHub: 'https://github.com/janedev' },
      {
        'orbit-sim': 'https://github.com/janedev/orbit-sim',
        'Spin glasses in 2D': 'https://doi.org/10.1/x',
      }
    );
    expect(out).toContain('[GitHub](https://github.com/janedev)'); // contact line
    expect(out).toContain('[orbit-sim](https://github.com/janedev/orbit-sim)'); // project bullet
    expect(out).toContain('[Spin glasses in 2D](https://doi.org/10.1/x)'); // publication bullet
  });

  it('body-link injection is idempotent and skips already-linked spans', () => {
    const text = '• orbit-sim — a simulator';
    const map = { 'orbit-sim': 'https://github.com/janedev/orbit-sim' };
    const once = injectLinksIntoGeneratedText(text, {}, map);
    const twice = injectLinksIntoGeneratedText(once, {}, map);
    expect(once).toContain('[orbit-sim](https://github.com/janedev/orbit-sim)');
    expect(twice).toBe(once);
  });

  it('does not inject body links when no bodyMap is passed (cover-letter path)', () => {
    const text = '• orbit-sim — a simulator';
    expect(injectLinksIntoGeneratedText(text, { GitHub: 'https://github.com/x' })).toBe(text);
  });
});
