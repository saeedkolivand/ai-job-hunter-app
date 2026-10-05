import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import {
  BASELINE_HEADER,
  BASELINE_REL,
  countCodeLines,
  evaluate,
  formatBaseline,
  HARD_CAP,
  parseBaseline,
} from './check-ts-size.mjs';

// Every expected number below is hand-counted from the fixture source, never
// derived from the function under test. The ratchet cases use the literal 300
// / 301 boundary, so they also pin HARD_CAP itself.

describe('countCodeLines', () => {
  it('counts nothing for an empty file or one with only blank lines', () => {
    expect(countCodeLines('', 'x.ts')).toBe(0);
    expect(countCodeLines('\n\n  \n\t\n', 'x.ts')).toBe(0);
  });

  it('does not count blank lines', () => {
    expect(countCodeLines('const a = 1;\n\n\nconst b = 2;\n', 'x.ts')).toBe(2);
  });

  it('does not count comment-only `//` lines', () => {
    expect(countCodeLines('// head\nconst a = 1;\n  // indented\n', 'x.ts')).toBe(1);
  });

  it("keeps a multi-line comment's newlines, so code on both sides still counts", () => {
    expect(countCodeLines('const a = 1; /* x\n y */ const b = 2;\n', 'x.ts')).toBe(2);
  });

  it('does not count a multi-line block comment', () => {
    const src = ['/*', ' * one', ' * two', ' */', 'const a = 1;', ''].join('\n');
    expect(countCodeLines(src, 'x.ts')).toBe(1);
  });

  it('does not count a JSDoc block', () => {
    const src = ['/**', ' * Adds.', ' * @param a first', ' */', 'export const add = 1;', ''].join(
      '\n'
    );
    expect(countCodeLines(src, 'x.ts')).toBe(1);
  });

  it('counts a line of code with a trailing comment', () => {
    expect(countCodeLines('const a = 1; // note\nconst b = 2; /* note */\n', 'x.ts')).toBe(2);
  });

  it('counts a line where code follows a block comment, including its closing line', () => {
    expect(
      countCodeLines('/* lead */ const a = 1;\n/* open\n close */ const b = 2;\n', 'x.ts')
    ).toBe(2);
  });

  it('counts a final line that has no trailing newline', () => {
    expect(countCodeLines('const a = 1;\nconst b = 2;', 'x.ts')).toBe(2);
  });

  it('handles CRLF line endings', () => {
    expect(countCodeLines('const a = 1;\r\n// c\r\n\r\nconst b = 2;\r\n', 'x.ts')).toBe(2);
  });

  it("counts a line whose string contains '//' or '/*'", () => {
    // A scanner that read `/*` inside a string as a comment opener would swallow line 2.
    const src = "const url = 'http://example.com';\nconst open = '/*';\nconst close = '*/';\n";
    expect(countCodeLines(src, 'x.ts')).toBe(3);
  });

  it('counts every line of a template literal, even ones that look like comments', () => {
    const src = ['const t = `', '// not a comment', '/* nor this', '*/', '`;', ''].join('\n');
    expect(countCodeLines(src, 'x.ts')).toBe(5);
  });

  it('counts JSX text containing an apostrophe or `//` in a .tsx file', () => {
    const src = [
      'export const A = () => (',
      '  <div>',
      "    <p>Don't</p>",
      '    <p>// still text</p>',
      '  </div>',
      ');',
      '',
    ].join('\n');
    expect(countCodeLines(src, 'x.tsx')).toBe(6);
  });

  it('does not count comments in a .tsx file, but keeps the braces of a JSX comment line', () => {
    const src = [
      '// header',
      'export const A = () => (',
      '  <div>',
      '    {/* only a comment */}',
      '    <p>hi</p> {/* trailing */}',
      '  </div>',
      ');',
      '',
    ].join('\n');
    // Line 1 is free; lines 2-7 are code. The JSX comment line still has its `{}` and
    // the trailing-comment line still has its `<p>`, so both count.
    expect(countCodeLines(src, 'x.tsx')).toBe(6);
  });

  it('parses angle-bracket assertions in .ts, which would be JSX in .tsx', () => {
    expect(countCodeLines('const a = <string>b;\n', 'x.ts')).toBe(1);
  });

  it('throws on a file that does not parse, instead of returning 0', () => {
    expect(() => countCodeLines('const a = {\n', 'x.ts')).toThrow();
  });
});

describe('evaluate', () => {
  const baseline = { 'old.ts': 310 };

  it('passes when every file is at or under the cap and nothing is baselined', () => {
    expect(evaluate({ counts: { 'a.ts': 1, 'b.tsx': 299 }, baseline: {} })).toEqual([]);
  });

  it('accepts a file at exactly the cap and rejects one line over', () => {
    expect(evaluate({ counts: { 'a.ts': 300 }, baseline: {} })).toEqual([]);
    expect(evaluate({ counts: { 'a.ts': 301 }, baseline: {} })).toEqual([
      'a.ts: 301 LOC — new file over the 300-line cap, split it',
    ]);
  });

  it('fails a baselined file that grew', () => {
    expect(evaluate({ counts: { 'old.ts': 311 }, baseline })).toEqual([
      'old.ts: 311 LOC — grew past its baseline of 310',
    ]);
  });

  it('allows a baselined file that shrank but is still over the cap', () => {
    expect(evaluate({ counts: { 'old.ts': 305 }, baseline })).toEqual([]);
  });

  it('allows a baselined file sitting exactly at its baseline', () => {
    expect(evaluate({ counts: { 'old.ts': 310 }, baseline })).toEqual([]);
  });

  it('fails a baselined file that now fits as a stale entry', () => {
    const stale = 'old.ts: now fits — remove it from scripts/data/ts-size-baseline.txt';
    expect(evaluate({ counts: { 'old.ts': 300 }, baseline })).toEqual([stale]);
    expect(evaluate({ counts: { 'old.ts': 12 }, baseline })).toEqual([stale]);
  });

  it('fails a baselined file that is no longer tracked as a stale entry', () => {
    expect(evaluate({ counts: { 'other.ts': 5 }, baseline })).toEqual([
      'old.ts: no longer exists — remove it from scripts/data/ts-size-baseline.txt',
    ]);
  });

  it('reports every violation, sorted by path', () => {
    const counts = { 'z-new.ts': 400, 'grew.ts': 520, 'fits.ts': 10, 'ok.ts': 350 };
    const base = { 'grew.ts': 500, 'fits.ts': 320, 'gone.ts': 330, 'ok.ts': 350 };
    expect(evaluate({ counts, baseline: base })).toEqual([
      'fits.ts: now fits — remove it from scripts/data/ts-size-baseline.txt',
      'gone.ts: no longer exists — remove it from scripts/data/ts-size-baseline.txt',
      'grew.ts: 520 LOC — grew past its baseline of 500',
      'z-new.ts: 400 LOC — new file over the 300-line cap, split it',
    ]);
  });
});

describe('parseBaseline', () => {
  it('reads <loc>\\t<path> rows and skips comments and blank lines', () => {
    const text = '# header\n\n400\ta/b.ts\n  \n301\tc.tsx\n';
    expect(parseBaseline(text)).toEqual({ 'a/b.ts': 400, 'c.tsx': 301 });
  });

  it('rejects a malformed row, naming its line', () => {
    expect(() => parseBaseline('# h\n400 a.ts\n')).toThrow(
      'scripts/data/ts-size-baseline.txt:2: malformed or duplicate line "400 a.ts"'
    );
    expect(() => parseBaseline('abc\ta.ts\n')).toThrow(/:1: malformed/);
  });

  it('rejects a path listed twice', () => {
    expect(() => parseBaseline('400\ta.ts\n390\ta.ts\n')).toThrow(/:2: malformed or duplicate/);
  });
});

describe('formatBaseline', () => {
  const counts = { 'b.ts': 301, 'a.ts': 400, 'c.ts': 300, 'd.ts': 5 };

  it('writes the header, then only over-cap files, sorted by path', () => {
    const out = formatBaseline(counts);
    expect(out.startsWith(BASELINE_HEADER)).toBe(true);
    expect(out.slice(BASELINE_HEADER.length)).toBe('400\ta.ts\n301\tb.ts\n');
  });

  it('writes a header made only of `#` comment lines', () => {
    const lines = BASELINE_HEADER.trimEnd().split('\n');
    expect(lines).toHaveLength(4);
    expect(lines.every((line) => line.startsWith('# '))).toBe(true);
  });

  it('round-trips through parseBaseline', () => {
    expect(parseBaseline(formatBaseline(counts))).toEqual({ 'a.ts': 400, 'b.ts': 301 });
  });
});

describe('the committed baseline', () => {
  const root = join(dirname(fileURLToPath(import.meta.url)), '..');
  const baseline = parseBaseline(readFileSync(join(root, BASELINE_REL), 'utf8'));
  const paths = Object.keys(baseline);

  it('lists only files over the cap', () => {
    expect(Object.values(baseline).every((loc) => loc > HARD_CAP)).toBe(true);
  });

  it('is sorted by path and lists only .ts/.tsx files', () => {
    expect(paths).toEqual([...paths].sort());
    expect(paths.every((p) => /\.tsx?$/.test(p) && !p.endsWith('.d.ts'))).toBe(true);
  });
});
