// Does a built file still load as an injected CLASSIC script?
//
// `chrome.scripting.executeScript({ files: [...] })` evaluates a file in the
// SCRIPT goal. Anything needing the module goal silently fails there, so
// `scripts/package.mjs` refuses to package a build where one of the injected
// entries has stopped being a classic script.
//
// Zero dependencies on purpose: this runs in the packaging step, so pulling a
// parser in would put a third-party package on the release path.

import { Script } from 'node:vm';

/**
 * Blank out comments and string / template / regex literals, keeping the source
 * the same length (and its newlines) so what is left lines up with the original.
 *
 * This is ONE left-to-right scan with an explicit state — not a sequence of
 * independent regex replacements. That distinction is the whole point: the
 * previous implementation stripped literals with a regex per literal kind, and a
 * single apostrophe (in a comment, or unbalanced inside a string table)
 * desynchronised the single-quote pass, which then swallowed everything up to
 * the next apostrophe. A scanner cannot desynchronise that way, because it only
 * ever leaves a literal through that literal's own terminator.
 *
 * Two deliberate bounds, neither of which the `Script` compile below depends on:
 *   - a template literal is blanked whole, `${...}` expressions included;
 *   - a regex literal is terminated at a newline (it cannot span one), so a
 *     misread division operator can affect at most the rest of that line.
 *
 * The second bound is only worth anything because the injected entries are built
 * UNMINIFIED (`minify: false` in `vite.config.mts`; 130-842 lines each). On a
 * minified one-liner "the rest of that line" is the rest of the file, and the
 * bound says nothing.
 *
 * Neither bound is free: two shapes are flagged that should not be. Both fail in
 * the release-BLOCKING direction rather than the silent-miss one, and both are
 * pinned as tests so a future fix flips them consciously:
 *   - a NESTED template (a `${...}` that itself holds a template) pairs the
 *     outer backtick with the INNER opening backtick, so the inner template's
 *     text is left to be scanned as code;
 *   - a regex in statement position after `)` — `if (x) /import (w+)/.test(s);`
 *     — reads the `)` as an operand, so the slash is division and the regex body
 *     is scanned as code.
 * Neither shape appears in the injected sources today, and a build that grew one
 * would fail loudly instead of shipping.
 */
export function stripCommentsAndLiterals(src) {
  const out = src.split('');
  const blank = (from, to) => {
    for (let k = from; k < to && k < out.length; k += 1) {
      if (out[k] !== '\n') out[k] = ' ';
    }
  };

  let i = 0;
  // The last non-whitespace character of real code, plus the identifier it ends
  // (if any) — together these tell a regex literal from a division operator.
  let prevChar = '';
  let prevWord = '';

  while (i < src.length) {
    const c = src[i];
    const next = src[i + 1];

    if (c === '/' && next === '/') {
      let j = i + 2;
      while (j < src.length && src[j] !== '\n') j += 1;
      blank(i, j);
      i = j;
      continue;
    }

    if (c === '/' && next === '*') {
      let j = i + 2;
      while (j < src.length && !(src[j] === '*' && src[j + 1] === '/')) j += 1;
      j = Math.min(j + 2, src.length);
      blank(i, j);
      i = j;
      continue;
    }

    if (c === '"' || c === "'" || c === '`') {
      let j = i + 1;
      while (j < src.length) {
        if (src[j] === '\\') {
          j += 2;
          continue;
        }
        if (src[j] === c) break;
        // An unterminated quote must not eat the rest of the file.
        if (c !== '`' && src[j] === '\n') break;
        j += 1;
      }
      blank(i + 1, j);
      i = Math.min(j, src.length) + 1;
      prevChar = c;
      prevWord = '';
      continue;
    }

    if (c === '/' && startsRegexLiteral(prevChar, prevWord)) {
      let j = i + 1;
      let inCharacterClass = false;
      while (j < src.length) {
        const ch = src[j];
        if (ch === '\\') {
          j += 2;
          continue;
        }
        if (ch === '\n') break;
        if (ch === '[') inCharacterClass = true;
        else if (ch === ']') inCharacterClass = false;
        else if (ch === '/' && !inCharacterClass) break;
        j += 1;
      }
      blank(i + 1, j);
      i = Math.min(j, src.length) + 1;
      prevChar = '/';
      prevWord = '';
      continue;
    }

    if (!/\s/.test(c)) {
      prevChar = c;
      prevWord = /[\w$]/.test(c) ? prevWord + c : '';
    }
    i += 1;
  }

  return out.join('');
}

/**
 * Keywords a regex literal may directly follow. Without these, `return /re/…`
 * reads as division-after-an-identifier, and a regex whose body contains a quote
 * (`/['"]/`) would then be scanned as code — the exact desync this scanner
 * exists to avoid.
 */
const REGEX_ALLOWING_KEYWORDS = new Set([
  'return',
  'typeof',
  'instanceof',
  'in',
  'of',
  'new',
  'delete',
  'void',
  'case',
  'do',
  'else',
  'yield',
  'await',
]);

function startsRegexLiteral(prevChar, prevWord) {
  if (prevChar === '') return true; // start of file
  if (REGEX_ALLOWING_KEYWORDS.has(prevWord)) return true;
  // After an operand (identifier, number, `)`, `]`) a slash is division.
  // Everything else — operators, `(`, `,`, `{`, `}`, `;` — can start a regex.
  return !/[\w$)\]]/.test(prevChar);
}

/**
 * A dynamic `import(` that is a real keyword: not `obj.import(`, not
 * `someimport(`, and not text inside a comment or string (those are blanked
 * before this runs).
 *
 * The dot has to be the character IMMEDIATELY before the keyword, so `obj. import(`
 * — with a space — is flagged. Harmless: prettier never emits that spacing.
 */
export const DYNAMIC_IMPORT_RE = /(^|[^\w$.])import\s*\(/;

/** Does `src` contain a dynamic `import(...)` in code (not in a literal)? */
export function containsDynamicImport(src) {
  return DYNAMIC_IMPORT_RE.test(stripCommentsAndLiterals(src));
}

/**
 * `null` when the source is a usable classic script, otherwise why it is not.
 *
 * `new Script(src)` compiles in the SCRIPT goal — the same goal
 * `executeScript({ files })` uses — and throws
 * `SyntaxError: Cannot use import statement outside a module` on any ES module
 * syntax. That is not a heuristic standing in for the invariant; it IS the
 * invariant. Compile only, never run: undefined globals like `document` and
 * `chrome` are irrelevant because nothing executes.
 *
 * A DYNAMIC `import(...)` is the one case the compile cannot see, because it is
 * valid script syntax — it just resolves nothing once injected. That needs the
 * scan above.
 */
export function failsToCompileAsClassicScript(src) {
  try {
    new Script(src);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  if (containsDynamicImport(src)) {
    return 'contains a dynamic import(), which resolves nothing in an injected classic script';
  }
  return null;
}
