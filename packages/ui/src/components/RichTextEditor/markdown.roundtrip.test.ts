/**
 * Hard no-drift idempotency gate + targeted edit tests for the WYSIWYG
 * markdown round-trip (§ Verification in the WYSIWYG plan).
 *
 * REQUIREMENT (plan §C): serialize(parse(md)) === md byte-exact for every
 * unedited real document. Any failure here is a BUG in markdown.ts, not in
 * this file — do NOT weaken assertions to make a broken round-trip pass.
 *
 * Sections (this file: 2 + 3; fixtures in markdown.roundtrip/corpus.ts,
 * edits in markdown.roundtrip/edits.test.ts, edge cases in
 * markdown.roundtrip/edge.test.ts):
 *   1. Corpus fixtures — real + crafted full-document samples
 *   2. Hard no-drift gate — roundTrip(md) === md for every corpus entry
 *   3. splitPreserved / joinPreserved direct unit assertions
 *   4. Targeted edit tests — one programmatic PM change; rest unchanged
 *   5. Edge / negative cases
 */

import { describe, expect, it } from 'vitest';

import { joinPreserved, roundTrip, splitPreserved } from './markdown';
import { CORPUS } from './markdown.roundtrip/corpus';

// ── 2. Hard no-drift gate ────────────────────────────────────────────────────

describe('no-drift gate: roundTrip(md) === md byte-exact (full corpus)', () => {
  for (const [label, md] of Object.entries(CORPUS)) {
    it(label, () => {
      const result = roundTrip(md);
      // Strong assertion: exact string equality — no normalization tolerance.
      // If this fails, report actual vs expected in the assertion message so
      // the orchestrator can route a targeted fix to markdown.ts.
      expect(result).toBe(md);
    });
  }

  // Individual coverage assertions for the plan's explicit must-survive list.

  it('double-space job-entry form (a): 2+ spaces before date survive byte-exact', () => {
    // This is the headline risk: markdown parsers collapse runs of spaces.
    // The custom line-oriented parser must NOT do that.
    const md = 'Senior Engineer  Acme Corp  Jan 2020 – Present';
    expect(roundTrip(md)).toBe(md);
    // Verify both the 2-space AND 4-space variants.
    const md4 = 'Staff Engineer    Cloudflare    Mar 2018 – Dec 2020';
    expect(roundTrip(md4)).toBe(md4);
  });

  it('job-entry form (b): trailing parenthesized date survives byte-exact', () => {
    const md = 'Staff Engineer, Contoso (Mar 2018 – Dec 2019)';
    expect(roundTrip(md)).toBe(md);
  });

  it('job-entry form (c): pipe/middot-separated entry survives byte-exact', () => {
    const pipe = 'Junior Engineer | Widget Co | 2016 – 2018';
    expect(roundTrip(pipe)).toBe(pipe);
    const middot = 'Designer · Studio · 2020 – 2022';
    expect(roundTrip(middot)).toBe(middot);
  });

  it.each([
    [
      'inline bold in full-document context does not corrupt surrounding text',
      '## Summary\nEngineer with **10 years** of experience.\n\n## Skills\nRust, Go',
    ],
    [
      'inline italic in full-document context does not corrupt surrounding text',
      'Experience includes *distributed systems* and cloud platforms.',
    ],
    [
      'inline link on a contact header line survives byte-exact',
      'Alex Kim\nalex@example.com | [LinkedIn](https://linkedin.com/in/alex) | [GitHub](https://github.com/alex)',
    ],
    [
      'ALL-CAPS banner heading survives byte-exact (not re-marked as ## heading)',
      'PROFESSIONAL EXPERIENCE\n\nSenior Engineer  Acme  2020 – Present',
    ],
    [
      'known section name heading (Experience, Skills) survives byte-exact',
      'Experience\n\nSenior Engineer  Acme  2020 – Present\n\nSkills\nRust, Go',
    ],
    [
      'custom ## heading survives byte-exact',
      '## Side Projects\n- Built a CLI tool\n- Contributed to open source',
    ],
    [
      'H3 subheading (###) survives byte-exact',
      '## Projects\n### Open Source\n- rust-analyzer\n- tokio',
    ],
    ['flat bullet list survives byte-exact', '## Skills\n- TypeScript\n- Rust\n- Go\n- PostgreSQL'],
    [
      'blank lines between sections preserved exactly (not added or removed)',
      '## Summary\nGreat candidate.\n\n## Experience\nSenior Engineer\n\n## Skills\nRust',
    ],
    [
      'name + contact header block survives byte-exact',
      'Jane Doe\njane.doe@example.com | +31 6 12345678 | linkedin.com/in/janedoe',
    ],
  ])('%s', (_name, md) => {
    expect(roundTrip(md)).toBe(md);
  });
});

// ── 3. splitPreserved / joinPreserved direct unit assertions ─────────────────

describe('splitPreserved / joinPreserved: direct unit assertions', () => {
  const BODY =
    '## Summary\nGreat candidate.\n\n## Experience\nSenior Engineer  Acme  2020 – Present\n- Did the thing.';
  const LINK_BLOCK =
    '\n---\n- [LinkedIn](https://linkedin.com/in/x)\n- [GitHub](https://github.com/x)\n- [Portfolio](https://example.com)';

  it('tail is detected: splitPreserved returns the link block as tail', () => {
    const { tail } = splitPreserved(BODY + LINK_BLOCK);
    expect(tail).toBe(LINK_BLOCK);
  });

  it('body excludes the link block entirely', () => {
    const { body } = splitPreserved(BODY + LINK_BLOCK);
    expect(body).toBe(BODY);
    expect(body).not.toContain('---');
    expect(body).not.toContain('[LinkedIn]');
  });

  it('joinPreserved restores the original document byte-exact', () => {
    const full = BODY + LINK_BLOCK;
    const { body, tail } = splitPreserved(full);
    expect(joinPreserved(body, tail)).toBe(full);
  });

  it('empty tail: no link block → tail is empty string', () => {
    const { body, tail } = splitPreserved(BODY);
    expect(body).toBe(BODY);
    expect(tail).toBe('');
  });

  it('joinPreserved with empty tail returns body unchanged', () => {
    expect(joinPreserved(BODY, '')).toBe(BODY);
  });

  it('link block with a single entry is still detected', () => {
    const single = BODY + '\n---\n- [LinkedIn](https://linkedin.com/in/x)';
    const { body, tail } = splitPreserved(single);
    expect(body).toBe(BODY);
    expect(tail).toBe('\n---\n- [LinkedIn](https://linkedin.com/in/x)');
  });

  it('uses LAST ---  separator (body may contain an earlier --- that is not a link block)', () => {
    // A --- in the body that is followed by non-link lines should NOT be the tail.
    const withEarlyRule = 'Intro text\n---\nNot a link line\n\n' + BODY + LINK_BLOCK;
    const { body, tail } = splitPreserved(withEarlyRule);
    // The link block is at the end — that one should be detected.
    expect(tail).toBe(LINK_BLOCK);
    expect(body).toBe('Intro text\n---\nNot a link line\n\n' + BODY);
  });

  it('a --- block NOT followed exclusively by link lines is NOT treated as tail', () => {
    const noTail = BODY + '\n---\nSome prose that is not a link line';
    const { tail } = splitPreserved(noTail);
    expect(tail).toBe('');
  });

  it('roundTrip leaves the held-out link block verbatim — byte-exact', () => {
    const full = BODY + LINK_BLOCK;
    expect(roundTrip(full)).toBe(full);
  });

  it('tail including trailing newline survives round-trip byte-exact', () => {
    // Some generated docs end with a newline after the last link line.
    const withTrailingNL = BODY + LINK_BLOCK + '\n';
    // splitPreserved should still detect the block; the trailing newline is
    // an empty line which is allowed by the validator (length === 0 is ok).
    const result = roundTrip(withTrailingNL);
    expect(result).toBe(withTrailingNL);
  });
});
