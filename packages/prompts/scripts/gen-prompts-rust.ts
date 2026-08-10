/**
 * Prompt -> Rust content-validator lexicon codegen.
 *
 * The content validator's `voice.ai_tell_lexical` / `voice.ai_tell_prose` /
 * `voice.template_opener` checks
 * (`apps/desktop/src-tauri/src/validate/content/lexicon.rs`) must ban exactly
 * what the generation prompt's natural-voice ruleset
 * (`packages/prompts/src/generate/natural-voice/natural-voice.ts`) instructs
 * the model to avoid — a validator checking a different list than the prompt
 * used is a false-positive machine. This generates the Rust lexicon module
 * directly from natural-voice.ts's own exported word-list arrays, so the two
 * can never drift.
 *
 * Run `pnpm gen:prompts` to regenerate, or `pnpm gen:prompts:check` to fail
 * when the committed output is stale (used in CI). Mirrors
 * `packages/shared/scripts/gen-ipc-rust.ts`.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  AI_TELL_LEXICAL_WORDS_DE,
  AI_TELL_LEXICAL_WORDS_EN,
  AI_TELL_PROSE_WORDS_DE,
  AI_TELL_PROSE_WORDS_EN,
  TEMPLATE_OPENERS_DE,
  TEMPLATE_OPENERS_EN,
} from '../src/generate/natural-voice/natural-voice.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, '../../..');
const OUT_FILE = 'apps/desktop/src-tauri/src/validate/content/lexicon.rs';

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
 * curated `"en"`/`"de"` lists. Every OTHER language returns an EMPTY slice,
 * never the English list — `natural-voice.ts` sends an uncurated language a
 * generic, wordless directive (no word list at all; see its
 * `genericAntiAiTellLexical`/`genericAntiAiTellProse`), so falling back to
 * the English words here would flag a language the prompt never told to
 * avoid them (MEDIUM fix, PR #963 round 5).
 */
export function rustLookupFn(
  fnName: string,
  doc: string,
  enConst: string,
  deConst: string
): string {
  return `${doc}
pub fn ${fnName}(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => ${deConst},
        "en" => ${enConst},
        // Every other language gets the prompt's generic, wordless directive
        // (see natural-voice.ts's genericAntiAiTellLexical/Prose) — there is
        // no curated list to check it against.
        _ => &[],
    }
}`;
}

function generate(): string {
  const body = [
    rustLookupFn(
      'ai_tell_lexical',
      '/// Word/phrase bans safe inside a résumé bullet — the lexical tier of\n' +
        '/// `antiAiTellLexical()`. No prose-flow rules here.',
      'AI_TELL_LEXICAL_EN',
      'AI_TELL_LEXICAL_DE'
    ),
    rustLookupFn(
      'ai_tell_prose',
      '/// Prose-only patterns from `antiAiTellProse()` — the hedging preambles and\n' +
        '/// stock transitions it bans as PHRASES, wherever they appear. Only meaningful\n' +
        '/// for connected writing (cover letters, summaries), never for a bullet.\n' +
        '///\n' +
        "/// The prompt's CONSTRUCTION-dependent prose rules (negative parallelism,\n" +
        '/// superficial "-ing" openers/tails) are deliberately absent: a substring\n' +
        '/// check cannot tell the banned construction from an ordinary sentence that\n' +
        '/// happens to contain the same words, so it would flag prose the prompt\n' +
        "/// permits. They stay prompt-only — see `AI_TELL_PROSE_WORDS_EN`'s doc in\n" +
        '/// natural-voice.ts.',
      'AI_TELL_PROSE_EN',
      'AI_TELL_PROSE_DE'
    ),
    rustLookupFn(
      'template_openers',
      '/// Stock cover-letter openers — the phrases a letter that could have been\n' +
        '/// addressed to anyone starts with.',
      'TEMPLATE_OPENERS_EN',
      'TEMPLATE_OPENERS_DE'
    ),
    rustArray('AI_TELL_LEXICAL_EN', AI_TELL_LEXICAL_WORDS_EN),
    rustArray('AI_TELL_LEXICAL_DE', AI_TELL_LEXICAL_WORDS_DE),
    rustArray('AI_TELL_PROSE_EN', AI_TELL_PROSE_WORDS_EN),
    rustArray('AI_TELL_PROSE_DE', AI_TELL_PROSE_WORDS_DE),
    rustArray('TEMPLATE_OPENERS_EN', TEMPLATE_OPENERS_EN),
    rustArray('TEMPLATE_OPENERS_DE', TEMPLATE_OPENERS_DE),
  ].join('\n\n');

  return [
    '// @generated by pnpm gen:prompts — DO NOT EDIT BY HAND.',
    '//',
    '//! Anti-AI-tell vocabulary, mirrored from the prompt side.',
    '//!',
    '//! Source of truth: `packages/prompts/src/generate/natural-voice/natural-voice.ts`',
    '//! (the `AI_TELL_LEXICAL_WORDS_*` / `AI_TELL_PROSE_WORDS_*` / `TEMPLATE_OPENERS_*`',
    '//! arrays) — the same bans the generation prompt hands the model, so the',
    '//! validator checks exactly what the prompt asked for. Run `pnpm gen:prompts`',
    '//! to regenerate after editing those arrays.',
    '//!',
    '//! `lang` is an ISO-639-1 code. `"en"`/`"de"` return their curated lists;',
    '//! every OTHER language returns an EMPTY slice, never the English list —',
    '//! `natural-voice.ts` sends an uncurated language a generic, wordless',
    '//! directive instead (see its `genericAntiAiTellLexical`/',
    '//! `genericAntiAiTellProse`), so falling back to English words here would',
    '//! flag a language the prompt never told to avoid them. Entries are',
    '//! lowercase and matched with word boundaries (see `super::contains_phrase`),',
    '//! so `vital` never fires on `revitalize`.',
    '',
    body,
    '',
  ].join('\n');
}

function main(): void {
  const check = process.argv.includes('--check');
  const target = join(REPO_ROOT, OUT_FILE);
  const next = generate();

  if (check) {
    let current: string;
    try {
      current = readFileSync(target, 'utf8');
    } catch {
      current = '';
    }
    if (current !== next) {
      console.error(`✗ stale: ${OUT_FILE} — run \`pnpm gen:prompts\``);
      process.exit(1);
    }
    console.log('✓ prompts codegen output is up to date');
  } else {
    writeFileSync(target, next);
    console.log(`✓ wrote ${OUT_FILE}`);
  }
}

// Only run when executed directly (`tsx scripts/gen-prompts-rust.ts`), not when
// imported by a test for its pure helpers (`hasRustUnsafeChar`/`rustArray`) — a
// bare top-level call would otherwise overwrite `lexicon.rs` as a side effect
// of running the test suite.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
