/**
 * Unit coverage for the pure helpers in gen-prompts-rust.ts (ai-provider-expert
 * L-1). The script itself only runs its `main()` when invoked directly (see the
 * `import.meta.url` guard at the bottom of that file), so importing it here for
 * `hasControlChar`/`rustArray` does not write to `lexicon.rs` as a side effect.
 */
import { describe, expect, it } from 'vitest';

import { hasControlChar, rustArray } from './gen-prompts-rust.js';

const NEWLINE = String.fromCharCode(10);
const TAB = String.fromCharCode(9);
const NUL = String.fromCharCode(0);
const DEL = String.fromCharCode(127);

describe('hasControlChar', () => {
  it('is false for ordinary words and phrases', () => {
    expect(hasControlChar('leverage')).toBe(false);
    expect(hasControlChar('it is not about')).toBe(false);
    expect(hasControlChar('mit großem interesse')).toBe(false);
    expect(hasControlChar('')).toBe(false);
  });

  it('is true for a newline, tab, null byte, or DEL', () => {
    expect(hasControlChar(`bad${NEWLINE}entry`)).toBe(true);
    expect(hasControlChar(`bad${TAB}entry`)).toBe(true);
    expect(hasControlChar(`bad${NUL}entry`)).toBe(true);
    expect(hasControlChar(`bad${DEL}entry`)).toBe(true);
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
