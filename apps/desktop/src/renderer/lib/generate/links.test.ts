import { describe, expect, it } from 'vitest';

import type { ContactProfile } from '@ajh/shared';

import { buildLinkSuggestions } from './links';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a minimal source-resume string whose trailing `\n---\n` reference block
 * parseLinkBlock() recognises.  Entries should be raw `[Label](url)` strings.
 */
function withLinkBlock(body: string, entries: string[]): string {
  if (entries.length === 0) return body;
  return body + '\n---\n' + entries.map((e) => `- ${e}`).join('\n');
}

// ---------------------------------------------------------------------------
// 1. Profile-only builds
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — ContactProfile fields', () => {
  it('maps linkedin → {label: LinkedIn, url}', () => {
    const profile: ContactProfile = { linkedin: 'https://linkedin.com/in/jane' };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toContainEqual({ label: 'LinkedIn', url: 'https://linkedin.com/in/jane' });
  });

  it('maps github → {label: GitHub, url}', () => {
    const profile: ContactProfile = { github: 'https://github.com/jane' };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toContainEqual({ label: 'GitHub', url: 'https://github.com/jane' });
  });

  it('maps website → {label: Website, url}', () => {
    const profile: ContactProfile = { website: 'https://janedoe.dev' };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toContainEqual({ label: 'Website', url: 'https://janedoe.dev' });
  });

  it('maps email → {label: Email, url: mailto:<addr>}', () => {
    const profile: ContactProfile = { email: 'jane@example.com' };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toContainEqual({ label: 'Email', url: 'mailto:jane@example.com' });
  });

  it('includes extraLinks with their own label/url', () => {
    const profile: ContactProfile = {
      extraLinks: [
        { label: 'Portfolio', url: 'https://portfolio.example.com' },
        { label: 'Blog', url: 'https://blog.example.com' },
      ],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toContainEqual({ label: 'Portfolio', url: 'https://portfolio.example.com' });
    expect(result).toContainEqual({ label: 'Blog', url: 'https://blog.example.com' });
  });

  it('returns all five named fields when all are set', () => {
    const profile: ContactProfile = {
      linkedin: 'https://linkedin.com/in/jane',
      github: 'https://github.com/jane',
      website: 'https://janedoe.dev',
      email: 'jane@example.com',
      extraLinks: [{ label: 'Portfolio', url: 'https://portfolio.example.com' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    const labels = result.map((s) => s.label);
    expect(labels).toContain('LinkedIn');
    expect(labels).toContain('GitHub');
    expect(labels).toContain('Website');
    expect(labels).toContain('Email');
    expect(labels).toContain('Portfolio');
  });
});

// ---------------------------------------------------------------------------
// 2. In-doc markdown link harvest
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — in-doc harvest from docValue', () => {
  it('extracts inline [label](url) links from docValue', () => {
    const docValue =
      'See my [Portfolio](https://portfolio.example.com) and [Blog](https://blog.example.com).';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toContainEqual({ label: 'Portfolio', url: 'https://portfolio.example.com' });
    expect(result).toContainEqual({ label: 'Blog', url: 'https://blog.example.com' });
  });

  it('harvests multiple links from a complex markdown document', () => {
    const docValue = [
      '# Jane Doe',
      'Connect via [LinkedIn](https://linkedin.com/in/jane).',
      'Code at [GitHub](https://github.com/jane).',
    ].join('\n');
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toContainEqual({ label: 'LinkedIn', url: 'https://linkedin.com/in/jane' });
    expect(result).toContainEqual({ label: 'GitHub', url: 'https://github.com/jane' });
  });

  it('returns [] for empty docValue and no profile', () => {
    const result = buildLinkSuggestions({ contactProfile: null, docValue: '' });
    expect(result).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 3. sourceResume link-block branch
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — sourceResume link block', () => {
  it('includes contact links extracted from a source resume link block', () => {
    const sourceResume = withLinkBlock('Jane Doe\njane@example.com', [
      '[LinkedIn](https://linkedin.com/in/jane)',
      '[GitHub](https://github.com/jane)',
    ]);
    const result = buildLinkSuggestions({ contactProfile: null, docValue: '', sourceResume });
    const urls = result.map((s) => s.url);
    expect(urls).toContain('https://linkedin.com/in/jane');
    expect(urls).toContain('https://github.com/jane');
  });

  it('includes body links (project/deep links) from the source resume block', () => {
    const sourceResume = withLinkBlock('Jane Doe', [
      '[LinkedIn](https://linkedin.com/in/jane)',
      '[orbit-sim](https://github.com/jane/orbit-sim)',
    ]);
    const result = buildLinkSuggestions({ contactProfile: null, docValue: '', sourceResume });
    const urls = result.map((s) => s.url);
    // orbit-sim is a body link (deep repo, 2 path segments) — must appear
    expect(urls).toContain('https://github.com/jane/orbit-sim');
  });

  it('returns [] when sourceResume has no link block', () => {
    const result = buildLinkSuggestions({
      contactProfile: null,
      docValue: '',
      sourceResume: 'Just a plain resume with no separator',
    });
    expect(result).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 4. De-duplication by normalized URL — label precedence
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — de-dup by normalized URL', () => {
  it('ContactProfile label beats in-doc anchor for same URL (trailing slash difference)', () => {
    // Profile has the canonical form; in-doc has a trailing slash variant.
    const profile: ContactProfile = { linkedin: 'https://linkedin.com/in/jane' };
    const docValue = 'See [My Profile](https://linkedin.com/in/jane/).';
    const result = buildLinkSuggestions({ contactProfile: profile, docValue });
    // Only one entry for this URL — the profile wins.
    const linkedinEntries = result.filter(
      (s) => s.url === 'https://linkedin.com/in/jane' || s.url === 'https://linkedin.com/in/jane/'
    );
    expect(linkedinEntries).toHaveLength(1);
    expect(linkedinEntries[0]?.label).toBe('LinkedIn');
  });

  it('ContactProfile label beats in-doc anchor for same URL (different-case host)', () => {
    const profile: ContactProfile = { website: 'https://janedoe.dev/about' };
    const docValue = '[About Me](https://JANEDOE.DEV/about)';
    const result = buildLinkSuggestions({ contactProfile: profile, docValue });
    const aboutEntries = result.filter((s) => s.url.toLowerCase().includes('janedoe.dev/about'));
    expect(aboutEntries).toHaveLength(1);
    expect(aboutEntries[0]?.label).toBe('Website');
  });

  it('does not de-dup genuinely different URLs', () => {
    const profile: ContactProfile = {
      linkedin: 'https://linkedin.com/in/jane',
      github: 'https://github.com/jane',
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result.length).toBeGreaterThanOrEqual(2);
  });

  it('mailto: de-dups by normalized address (case-insensitive)', () => {
    const profile: ContactProfile = { email: 'Jane@Example.COM' };
    const docValue = '[Contact](mailto:jane@example.com)';
    const result = buildLinkSuggestions({ contactProfile: profile, docValue });
    const mailtoEntries = result.filter((s) => s.url.toLowerCase().startsWith('mailto:'));
    expect(mailtoEntries).toHaveLength(1);
    // profile entry comes first → label is 'Email'
    expect(mailtoEntries[0]?.label).toBe('Email');
  });
});

// ---------------------------------------------------------------------------
// 5. Scheme filter — only http / https / mailto survive
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — scheme filter', () => {
  it('drops javascript: links found in docValue', () => {
    const docValue = '[Click](javascript:alert(1))';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toEqual([]);
  });

  it('drops ftp: links found in docValue', () => {
    const docValue = '[Files](ftp://files.example.com/resume.pdf)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toEqual([]);
  });

  it('drops a javascript: URL in an extraLink', () => {
    const profile: ContactProfile = {
      extraLinks: [{ label: 'XSS', url: 'javascript:void(0)' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toEqual([]);
  });

  it('drops an ftp: URL in an extraLink', () => {
    const profile: ContactProfile = {
      extraLinks: [{ label: 'FTP', url: 'ftp://ftp.example.com' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toEqual([]);
  });

  it('keeps http: links (not just https)', () => {
    const docValue = '[Insecure](http://example.com/page)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toContainEqual({ label: 'Insecure', url: 'http://example.com/page' });
  });

  it('keeps https: links', () => {
    const docValue = '[Secure](https://example.com/page)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toContainEqual({ label: 'Secure', url: 'https://example.com/page' });
  });

  it('keeps mailto: links', () => {
    const docValue = '[Email me](mailto:hi@example.com)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toContainEqual({ label: 'Email me', url: 'mailto:hi@example.com' });
  });
});

// ---------------------------------------------------------------------------
// 6. Empty / degenerate inputs
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — empty inputs', () => {
  it('returns [] with no profile, empty docValue, no sourceResume', () => {
    expect(buildLinkSuggestions({ contactProfile: null, docValue: '' })).toEqual([]);
  });

  it('returns [] when contactProfile is undefined', () => {
    expect(buildLinkSuggestions({ contactProfile: undefined, docValue: '' })).toEqual([]);
  });

  it('drops entries with empty label', () => {
    const profile: ContactProfile = {
      extraLinks: [{ label: '', url: 'https://example.com' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toEqual([]);
  });

  it('drops entries with empty url', () => {
    const profile: ContactProfile = {
      extraLinks: [{ label: 'Empty URL', url: '' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toEqual([]);
  });

  it('drops entries with whitespace-only label', () => {
    const docValue = '[   ](https://example.com)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue });
    expect(result).toEqual([]);
  });

  it('drops entries with whitespace-only url', () => {
    const profile: ContactProfile = {
      extraLinks: [{ label: 'Bad', url: '   ' }],
    };
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '' });
    expect(result).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 7. Precedence order: profile → sourceResume → in-doc
// ---------------------------------------------------------------------------

describe('buildLinkSuggestions — precedence order', () => {
  it('sourceResume label wins over in-doc when same URL (source comes before in-doc)', () => {
    // Source resume has a label; in-doc uses a different label for the same URL.
    const sourceResume = withLinkBlock('Body', ['[GitHub](https://github.com/jane)']);
    // docValue uses a different anchor text for the same URL
    const docValue = '[My Code](https://github.com/jane)';
    const result = buildLinkSuggestions({ contactProfile: null, docValue, sourceResume });
    const entry = result.find((s) => s.url === 'https://github.com/jane');
    expect(entry).toBeDefined();
    // sourceResume getLinkMap fires first → "GitHub" label wins
    expect(entry?.label).toBe('GitHub');
  });

  it('profile wins over sourceResume when same URL', () => {
    const profile: ContactProfile = { github: 'https://github.com/jane' };
    const sourceResume = withLinkBlock('Body', ['[Some Other Label](https://github.com/jane)']);
    const result = buildLinkSuggestions({ contactProfile: profile, docValue: '', sourceResume });
    const entry = result.find((s) => s.url === 'https://github.com/jane');
    expect(entry?.label).toBe('GitHub');
  });
});
