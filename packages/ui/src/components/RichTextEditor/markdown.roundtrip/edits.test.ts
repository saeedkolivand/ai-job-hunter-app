/**
 * Targeted edit tests — parse a document → apply ONE programmatic edit →
 * docToMarkdown → assert (a) the intended change is present, (b) everything
 * else — especially job-entry date lines and the link block — is unchanged.
 *
 * Headless: ProseMirror Node API only, no live editor.
 */

import { describe, expect, it } from 'vitest';

import {
  docToMarkdown,
  getEditorSchema,
  joinPreserved,
  markdownToDoc,
  splitPreserved,
} from '../markdown';

describe('targeted edit tests: single edit, rest unchanged', () => {
  const schema = getEditorSchema();

  // Base document used for edit tests — contains all risk elements.
  const BASE_MD = [
    'Jordan Lee',
    'jordan@example.com | [LinkedIn](https://linkedin.com/in/jordan)',
    '',
    '## Experience',
    '',
    'Senior Engineer  Acme Corp  Jan 2020 – Present',
    '- Led the platform team.',
    '- Shipped 4 major features.',
    '',
    'Staff Engineer, Contoso (Mar 2018 – Dec 2019)',
    '- Rewrote billing system.',
    '',
    'Junior Engineer | Widget Co | 2016 – 2018',
    '- Built REST APIs.',
    '',
    '## Skills',
    '- Go, Rust, TypeScript',
  ].join('\n');
  const LINK_BLOCK =
    '\n---\n- [LinkedIn](https://linkedin.com/in/jordan)\n- [GitHub](https://github.com/jordan)';
  const FULL_MD = BASE_MD + LINK_BLOCK;

  /** Apply `change` to the body, re-parse, serialize, and re-attach the held-out tail. */
  function edit(change: (body: string) => string): string {
    const { body, tail } = splitPreserved(FULL_MD);
    return joinPreserved(docToMarkdown(markdownToDoc(change(body), schema)), tail);
  }

  /** All three job-entry date forms (byte-exact, incl. the 2-space form) + the link block. */
  function expectDatesAndLinkBlockIntact(result: string) {
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
    expect(result).toContain('Staff Engineer, Contoso (Mar 2018 – Dec 2019)');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    expect(result).toContain(LINK_BLOCK);
  }

  it('bolding a word: intended bold is present; link block and dates unchanged', () => {
    const result = edit((body) =>
      body.replace('Led the platform team.', '**Led** the platform team.')
    );

    expect(result).toContain('**Led** the platform team.');
    expectDatesAndLinkBlockIntact(result);
  });

  it('adding a bullet item: new item present; dates and link block unchanged', () => {
    const result = edit((body) =>
      body.replace(
        '## Skills\n- Go, Rust, TypeScript',
        '## Skills\n- Go, Rust, TypeScript\n- PostgreSQL, Redis'
      )
    );

    expect(result).toContain('- PostgreSQL, Redis');
    // Existing bullets unchanged.
    expect(result).toContain('- Go, Rust, TypeScript');
    expectDatesAndLinkBlockIntact(result);
  });

  it('inserting a link in body: link present; double-space dates and link block unchanged', () => {
    const result = edit((body) =>
      body.replace('Rewrote billing system.', 'Rewrote billing system using [Go](https://go.dev).')
    );

    expect(result).toContain('[Go](https://go.dev)');
    expect(result).toContain('Rewrote billing system using [Go](https://go.dev).');
    expectDatesAndLinkBlockIntact(result);
  });

  it('adding a ## heading: heading present; dates and link block unchanged', () => {
    const result = edit((body) => body + '\n\n## Certifications\nAWS Solutions Architect (2023)');

    expect(result).toContain('## Certifications');
    expect(result).toContain('AWS Solutions Architect (2023)');
    expectDatesAndLinkBlockIntact(result);
  });

  it('edit does not corrupt the contact header line', () => {
    // Simulate editing the summary (adding a sentence after it).
    const result = edit((body) =>
      body.replace('## Experience', '## Summary\nEngineered systems at scale.\n\n## Experience')
    );

    expect(result).toContain('jordan@example.com | [LinkedIn](https://linkedin.com/in/jordan)');
    expect(result).toContain(LINK_BLOCK);
  });

  it('italic applied: italic present; surrounding job-entry lines byte-exact', () => {
    const result = edit((body) => body.replace('Built REST APIs.', 'Built *REST* APIs.'));

    expect(result).toContain('Built *REST* APIs.');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    expect(result).toContain(LINK_BLOCK);
  });

  // Verify that the doc produced by markdownToDoc is round-trippable via
  // docToMarkdown independently from the full roundTrip helper.
  it('markdownToDoc → docToMarkdown round-trips the base document independently', () => {
    expect(edit((body) => body)).toBe(FULL_MD);
  });
});
