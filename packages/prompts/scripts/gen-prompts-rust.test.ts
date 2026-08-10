/**
 * Unit coverage for the pure helpers in gen-prompts-rust.ts (ai-provider-expert
 * L-1). The script itself only runs its `main()` when invoked directly (see the
 * `import.meta.url` guard at the bottom of that file), so importing it here for
 * `hasRustUnsafeChar`/`rustArray` does not write to `lexicon.rs` as a side effect.
 */
import { describe, expect, it } from 'vitest';

import { hasRustUnsafeChar, rustArray, rustLookupFn } from './gen-prompts-rust.js';

const NEWLINE = String.fromCharCode(10);
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
