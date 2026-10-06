/** Rust-source formatting helpers shared by the prompt codegen. */

/**
 * True when `entry` contains a character `JSON.stringify` would emit as a
 * `\uXXXX` escape: a C0 control character, DEL, or a LONE surrogate
 * (well-formed stringify, ES2019+, escapes unpaired surrogates the same way;
 * valid pairs come out as raw astral chars, which Rust accepts). `\uXXXX` is
 * valid JSON but NOT a valid Rust string escape (Rust needs the bracketed
 * `\u{XXXX}` form) — such a character surviving into an emitted array would
 * produce Rust source that fails to compile with a confusing "unknown
 * character escape" error far from its actual cause. Exported for its own
 * unit test.
 */
export function hasRustUnsafeChar(entry: string): boolean {
  for (let i = 0; i < entry.length; i += 1) {
    const code = entry.charCodeAt(i);
    if (code <= 0x1f || code === 0x7f) return true;
    if (code >= 0xd800 && code <= 0xdfff) {
      // NaN comparisons at end-of-string correctly read as "not a pair".
      const next = entry.charCodeAt(i + 1);
      if (code >= 0xdc00 || !(next >= 0xdc00 && next <= 0xdfff)) return true;
      i += 1; // valid surrogate pair — emitted as a raw astral char
    }
  }
  return false;
}

/**
 * One `&[&str]` const, formatted to match `cargo fmt`'s own choice: a single
 * line when the whole declaration fits within rustfmt's 100-col `max_width`,
 * else one entry per line (rustfmt's vertical list layout for the arrays this
 * module emits — words/short phrases, never short enough on average to
 * trigger rustfmt's separate horizontal-packing tactic). `cargo fmt --check`
 * (CI) is the backstop if a future word list ever lands outside that shape.
 *
 * Throws (rather than silently emitting invalid Rust) if any entry contains a
 * control character or lone surrogate — see {@link hasRustUnsafeChar}.
 */
export function rustArray(name: string, entries: readonly string[]): string {
  const offender = entries.find(hasRustUnsafeChar);
  if (offender !== undefined) {
    throw new Error(
      `gen-prompts-rust: ${name} entry ${JSON.stringify(offender)} contains a control ` +
        "character or lone surrogate — JSON.stringify's \\uXXXX escaping is not valid Rust " +
        'string-literal syntax (Rust needs \\u{XXXX}). Fix the entry in natural-voice.ts and rerun.'
    );
  }
  const items = entries.map((e) => JSON.stringify(e));
  const singleLine = `const ${name}: &[&str] = &[${items.join(', ')}];`;
  if (singleLine.length <= 100) return singleLine;
  return `const ${name}: &[&str] = &[\n${items.map((e) => `    ${e},`).join('\n')}\n];`;
}

/**
 * `pub fn <name>(lang: &str) -> &'static [&'static str]` dispatching to the
 * curated `"en"`/`"de"`/`"it"` lists. Every OTHER language returns an EMPTY
 * slice, never the English list — `natural-voice.ts` sends an uncurated
 * language a generic, wordless directive (no word list at all; see its
 * `genericAntiAiTellLexical`/`genericAntiAiTellProse`), so falling back to
 * the English words here would flag a language the prompt never told to
 * avoid them (MEDIUM fix, PR #963 round 5).
 */
export function rustLookupFn(
  fnName: string,
  doc: string,
  enConst: string,
  deConst: string,
  itConst: string
): string {
  return `${doc}
pub fn ${fnName}(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => ${deConst},
        "en" => ${enConst},
        "it" => ${itConst},
        // Every other language gets the prompt's generic, wordless directive
        // (see natural-voice.ts's genericAntiAiTellLexical/Prose) — there is
        // no curated list to check it against.
        _ => &[],
    }
}`;
}

/**
 * A Rust `&'static str` literal for `value`.
 *
 * Unlike {@link hasRustUnsafeChar}, newline/CR/tab are ALLOWED — the prompt
 * blocks are multi-line templates and `JSON.stringify` emits those three as
 * `\n`/`\r`/`\t`, which Rust accepts verbatim. Strips them before delegating
 * to {@link hasRustUnsafeChar}'s control-char/lone-surrogate walk (rather than
 * re-implementing it), so the two functions can never drift on what counts as
 * unsafe. Every OTHER control character (and a lone surrogate) throws rather
 * than emitting source that fails to compile far from its cause.
 */
export function rustStringLiteral(name: string, value: string): string {
  if (hasRustUnsafeChar(value.replace(/[\n\r\t]/g, ''))) {
    throw new Error(
      `gen-prompts-rust: ${name} contains a control character or lone surrogate — ` +
        "JSON.stringify's \\uXXXX escaping is not valid Rust string-literal syntax " +
        '(Rust needs \\u{XXXX}).'
    );
  }
  return JSON.stringify(value);
}

/**
 * One `pub const NAME: &str = "…";`, wrapped the way `cargo fmt` wraps it: a
 * single line while it fits rustfmt's 100-col `max_width`, otherwise the value
 * on its own 4-space-indented line (rustfmt cannot split a string literal, so
 * that is as far as it goes). Same tactic as `gen-ipc-rust.ts`'s
 * `genDateFilters`, and `cargo fmt --check` in CI is the backstop.
 */
export function rustStrConst(doc: string, name: string, value: string): string {
  const literal = rustStringLiteral(name, value);
  const singleLine = `pub const ${name}: &str = ${literal};`;
  const decl = singleLine.length <= 100 ? singleLine : `pub const ${name}: &str =\n    ${literal};`;
  return `${doc}\n${decl}`;
}
