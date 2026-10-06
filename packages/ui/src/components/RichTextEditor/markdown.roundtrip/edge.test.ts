/** Edge / negative cases for the markdown round-trip. */

import { describe, expect, it } from 'vitest';

import {
  docToMarkdown,
  getEditorSchema,
  markdownToDoc,
  roundTrip,
  splitPreserved,
} from '../markdown';
import { CORPUS_REAL_RESUME } from './corpus';

describe('edge and negative cases', () => {
  it('empty string: roundTrip returns empty string (no content)', () => {
    // Empty body → single empty paragraph → serializes as '' (one empty line = '').
    // This must not throw.
    expect(() => roundTrip('')).not.toThrow();
    expect(roundTrip('')).toBe('');
  });

  it('doc with NO link block: tail is empty string, body is the whole doc', () => {
    const md = '## Summary\nGreat candidate.\n\n## Skills\n- Rust\n- Go';
    const { body, tail } = splitPreserved(md);
    expect(tail).toBe('');
    expect(body).toBe(md);
    expect(roundTrip(md)).toBe(md);
  });

  it.each([
    ['·', 'middot', 'Role · Company · 2020 – Present', '\\·'],
    ['|', 'pipe', 'Role | Company | 2016 – 2018', '\\|'],
    ['@', 'email @', 'jane.doe@example.com | +31 6 12345678', '\\@'],
  ])('literal %s (%s) in body: NOT markdown-escaped on serialize', (_c, _n, md, escaped) => {
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).not.toContain(escaped);
  });

  it('literal ( and ) in body: NOT markdown-escaped on serialize', () => {
    const md = 'Staff Engineer, Contoso (Mar 2018 – Dec 2019)';
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).not.toContain('\\(');
    expect(roundTrip(md)).not.toContain('\\)');
  });

  it('em-dash (–) in date range: preserved verbatim', () => {
    const md = 'Senior Engineer  Acme Corp  Jan 2020 – Present';
    expect(roundTrip(md)).toBe(md);
    // Must be the en-dash U+2013, not a hyphen.
    expect(roundTrip(md)).toContain('–');
  });

  it.each([
    [
      'literal - at start of paragraph (not a bullet): NOT treated as bullet',
      'AWS Solutions Architect — Associate (2022)',
    ],
    [
      'consecutive blank lines: preserved as distinct block separators',
      // Two consecutive blank lines → two empty paragraphs → two blank lines.
      '## Section A\nContent.\n\n\n## Section B\nMore content.',
    ],
    [
      'link inside a bullet item: survives round-trip byte-exact',
      '## Projects\n- [rust-http-client](https://github.com/alexkim/rust-http): async Rust HTTP client.',
    ],
    [
      // Wiki-style URL containing a nested, balanced paren. The inline-link URL
      // pattern allows one level of nesting, so the URL is captured to its FINAL
      // ) rather than truncated at the first interior ).
      'link URL with a balanced (...) inside it survives byte-exact (no truncation)',
      '[C](https://en.wikipedia.org/wiki/C_(programming_language))',
    ],
    [
      // The balanced-paren URL must not greedily swallow a trailing ) that
      // belongs to surrounding prose.
      'link with nested-paren URL followed by literal text does not over-consume',
      'See [C](https://en.wikipedia.org/wiki/C_(programming_language)) (the language).',
    ],
    [
      'plain link followed by literal text containing ) is unaffected',
      'Built [Workers KV](https://developers.cloudflare.com/kv/) (fast).',
    ],
    [
      'bold inside a heading: survives round-trip byte-exact',
      '## Summary\n**Lead** engineer with 10 years of experience.',
    ],
    [
      'italic inside a bullet: survives round-trip byte-exact',
      '## Skills\n- *TypeScript*, Rust, Go',
    ],
    [
      'multiple links on one line (contact header): all survive byte-exact',
      'Alex Kim\nalex@example.com | [LinkedIn](https://linkedin.com/in/alexkim) | [GitHub](https://github.com/alexkim) | [Portfolio](https://alexkim.dev)',
    ],
  ])('%s', (_name, md) => {
    expect(roundTrip(md)).toBe(md);
  });

  it('splitPreserved: link block with empty trailing line still detected', () => {
    const body = '## Summary\nContent.';
    const tail = '\n---\n- [LinkedIn](https://linkedin.com/in/x)\n';
    const split = splitPreserved(body + tail);
    expect(split.body).toBe(body);
    expect(split.tail).toBe(tail);
  });

  it('getEditorSchema: returns a ProseMirror Schema with required node types', () => {
    const s = getEditorSchema();
    // The locked schema must have these nodes for the round-trip to work.
    expect(s.nodes['doc']).toBeDefined();
    expect(s.nodes['paragraph']).toBeDefined();
    expect(s.nodes['heading']).toBeDefined();
    expect(s.nodes['bulletList']).toBeDefined();
    expect(s.nodes['listItem']).toBeDefined();
    expect(s.marks['bold']).toBeDefined();
    expect(s.marks['italic']).toBeDefined();
    expect(s.marks['link']).toBeDefined();
    // Disabled nodes must NOT be in the schema (locked schema guarantee).
    expect(s.nodes['codeBlock']).toBeUndefined();
    expect(s.nodes['blockquote']).toBeUndefined();
    expect(s.nodes['orderedList']).toBeUndefined();
  });

  it('real fixture (resume.txt) round-trips via markdownToDoc → docToMarkdown independently', () => {
    // Test the individual functions, not just the high-level roundTrip helper.
    const { body, tail } = splitPreserved(CORPUS_REAL_RESUME);
    expect(tail).toBe(''); // real fixture has no link block — confirm assumption.
    const doc = markdownToDoc(body);
    const serialized = docToMarkdown(doc);
    expect(serialized).toBe(body);
  });
});
