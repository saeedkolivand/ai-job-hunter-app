/**
 * Unit coverage for the pure helpers in gen-prompts-rust.ts (ai-provider-expert
 * L-1). The script itself only runs its `main()` when invoked directly (see the
 * `import.meta.url` guard at the bottom of that file), so importing it here for
 * `hasRustUnsafeChar`/`rustArray` does not write to `lexicon.rs` as a side effect.
 */
import { describe, expect, it } from 'vitest';

import {
  hasRustUnsafeChar,
  rustArray,
  rustLookupFn,
  rustStrConst,
  rustStringLiteral,
} from './gen-prompts-rust.js';

const NEWLINE = String.fromCharCode(10);
const CR = String.fromCharCode(13);
const TAB = String.fromCharCode(9);
const NUL = String.fromCharCode(0);
const DEL = String.fromCharCode(127);
const LONE_HIGH = String.fromCharCode(0xd800);
const LONE_LOW = String.fromCharCode(0xdc00);

describe('hasRustUnsafeChar', () => {
  it('is false for ordinary words and phrases', () => {
    expect(hasRustUnsafeChar('leverage')).toBe(false);
    expect(hasRustUnsafeChar('it is not about')).toBe(false);
    expect(hasRustUnsafeChar('mit großem interesse')).toBe(false);
    expect(hasRustUnsafeChar('')).toBe(false);
  });

  it('is true for a newline, tab, null byte, or DEL', () => {
    expect(hasRustUnsafeChar(`bad${NEWLINE}entry`)).toBe(true);
    expect(hasRustUnsafeChar(`bad${TAB}entry`)).toBe(true);
    expect(hasRustUnsafeChar(`bad${NUL}entry`)).toBe(true);
    expect(hasRustUnsafeChar(`bad${DEL}entry`)).toBe(true);
  });

  it('is true for a lone surrogate (stringify escapes it as \\uXXXX)', () => {
    expect(hasRustUnsafeChar(`bad${LONE_HIGH}entry`)).toBe(true);
    expect(hasRustUnsafeChar(`bad${LONE_LOW}entry`)).toBe(true);
    expect(hasRustUnsafeChar(LONE_HIGH)).toBe(true); // high surrogate at end-of-string
  });

  it('is false for a well-formed astral pair (emitted as a raw char)', () => {
    expect(hasRustUnsafeChar('emoji 😀 entry')).toBe(false);
  });
});

describe('rustArray', () => {
  it('emits a single-line const when the declaration fits within 100 cols', () => {
    expect(rustArray('SHORT', ['a', 'b'])).toBe('const SHORT: &[&str] = &["a", "b"];');
  });

  it('wraps to one entry per line when the single-line form exceeds 100 cols', () => {
    const long = Array.from({ length: 12 }, (_, i) => `a reasonably long entry number ${i}`);
    const out = rustArray('LONG', long);
    expect(out.startsWith('const LONG: &[&str] = &[\n')).toBe(true);
    expect(out).toContain('    "a reasonably long entry number 0",\n');
    expect(out.endsWith('\n];')).toBe(true);
  });

  it('throws a clear, named error instead of emitting invalid Rust when an entry has a control char', () => {
    const bad = ['fine', `bad${NEWLINE}entry`];
    expect(() => rustArray('BAD', bad)).toThrowError(/BAD/);
    expect(() => rustArray('BAD', bad)).toThrowError(/control character/);
    expect(() => rustArray('BAD', bad)).toThrowError(/u\{XXXX\}/);
  });

  it('never throws for a control-char-free array, however long', () => {
    expect(() => rustArray('FINE', ['delve', 'leverage', 'robust'])).not.toThrow();
  });
});

describe('rustLookupFn', () => {
  /**
   * MEDIUM fix (PR #963 round 5): the generated dispatch used to fall back
   * to the EN const for every uncurated language (`_ => enConst`), so the
   * validator flagged English AI-tell words in, say, a French letter — but
   * `natural-voice.ts` sends French a generic, wordless directive with no
   * word list at all. Every non-`en`/`de` language must dispatch to an
   * EMPTY slice instead, matching the prompt's own behavior.
   */
  it('dispatches "de"/"en" to their curated consts and every other language to an empty slice, never the English fallback', () => {
    const out = rustLookupFn(
      'ai_tell_lexical',
      '/// doc',
      'AI_TELL_LEXICAL_EN',
      'AI_TELL_LEXICAL_DE'
    );
    expect(out).toContain('"de" => AI_TELL_LEXICAL_DE,');
    expect(out).toContain('"en" => AI_TELL_LEXICAL_EN,');
    expect(out).toContain('_ => &[],');
    expect(out).not.toMatch(/_\s*=>\s*AI_TELL_LEXICAL_EN/);
  });
});

describe('rustStringLiteral', () => {
  it('quotes and escapes a plain value the same way JSON.stringify does', () => {
    expect(rustStringLiteral('NAME', 'hello "world"')).toBe(JSON.stringify('hello "world"'));
    expect(rustStringLiteral('NAME', 'C:\\path\\to\\file')).toBe(
      JSON.stringify('C:\\path\\to\\file')
    );
  });

  it('allows newline, CR, and tab — unlike hasRustUnsafeChar', () => {
    const value = `line one${NEWLINE}line two${CR}${TAB}indented`;
    expect(hasRustUnsafeChar(value)).toBe(true); // the raw walk still bans them
    expect(rustStringLiteral('NAME', value)).toBe(JSON.stringify(value));
  });

  it('throws a clear, named error instead of emitting invalid Rust for any other control character', () => {
    expect(() => rustStringLiteral('NAME', `bad${NUL}entry`)).toThrowError(/NAME/);
    expect(() => rustStringLiteral('NAME', `bad${NUL}entry`)).toThrowError(/control character/);
    expect(() => rustStringLiteral('NAME', `bad${DEL}entry`)).toThrowError(/control character/);
  });

  it('throws for a lone high surrogate', () => {
    expect(() => rustStringLiteral('NAME', `bad${LONE_HIGH}entry`)).toThrowError(/NAME/);
    expect(() => rustStringLiteral('NAME', LONE_HIGH)).toThrowError(/lone surrogate/);
  });

  it('throws for a lone low surrogate', () => {
    expect(() => rustStringLiteral('NAME', `bad${LONE_LOW}entry`)).toThrowError(/lone surrogate/);
  });

  it('accepts a well-formed astral pair', () => {
    const value = 'emoji 😀 entry';
    expect(() => rustStringLiteral('NAME', value)).not.toThrow();
    expect(rustStringLiteral('NAME', value)).toBe(JSON.stringify(value));
  });
});

describe('rustStrConst', () => {
  it('emits a single-line const when the declaration fits within 100 cols', () => {
    expect(rustStrConst('/// doc', 'SHORT', 'hello')).toBe(
      '/// doc\npub const SHORT: &str = "hello";'
    );
  });

  it('wraps the value onto its own indented line when the single-line form exceeds 100 cols', () => {
    const long =
      'a reasonably long value, repeated a little more, so the line runs well past a hundred columns';
    const out = rustStrConst('/// doc', 'LONG', long);
    expect(out).toBe(`/// doc\npub const LONG: &str =\n    ${JSON.stringify(long)};`);
  });
});
