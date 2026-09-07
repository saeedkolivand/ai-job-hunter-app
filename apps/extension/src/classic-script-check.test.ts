/**
 * The gate that stops a build shipping an injected script the browser cannot
 * load.
 *
 * `scripts/package.mjs` refuses to package when one of the `executeScript`
 * entries has stopped being a classic script. This has been wrong in both
 * directions before, so both are pinned here:
 *
 *   - MISSING a real problem — the original regex stripped literals with one
 *     regex per literal kind, which desynchronised on a single apostrophe and
 *     swallowed everything after it, so a static `import` appended to five of
 *     the nine real built artifacts went undetected.
 *   - INVENTING one — scanning unstripped source flags `obj.import(` or the word
 *     inside a comment, which would fail a perfectly good release.
 */
import { describe, expect, it } from 'vitest';

import {
  containsDynamicImport,
  failsToCompileAsClassicScript,
  stripCommentsAndLiterals,
} from '../scripts/classic-script-check.mjs';

describe('stripCommentsAndLiterals', () => {
  it('keeps the source the same length, so offsets still line up', () => {
    const src = "const a = 'hello'; // note\nconst b = `t${x}`;\n";
    expect(stripCommentsAndLiterals(src)).toHaveLength(src.length);
  });

  it('preserves newlines so line numbers survive', () => {
    const src = '/* one\ntwo\nthree */\ncode;\n';
    const out = stripCommentsAndLiterals(src);
    expect(out.split('\n')).toHaveLength(src.split('\n').length);
    expect(out).toContain('code;');
  });

  it('leaves real code alone', () => {
    expect(stripCommentsAndLiterals('const x = foo(1, 2);')).toBe('const x = foo(1, 2);');
  });

  // The exact shape that broke the previous implementation: an apostrophe in a
  // comment opened a "string" that ran to the next apostrophe anywhere later in
  // the file, hiding whatever came between.
  it('does not let an apostrophe in a comment swallow the rest of the file', () => {
    const src = "// don't do this\nconst real = 1;\n// it's fine\nconst alsoReal = 2;\n";
    const out = stripCommentsAndLiterals(src);
    expect(out).toContain('const real = 1;');
    expect(out).toContain('const alsoReal = 2;');
  });

  it('does not let an escaped apostrophe inside a string swallow later code', () => {
    const src = "const s = 'it\\'s';\nconst real = 1;\n";
    expect(stripCommentsAndLiterals(src)).toContain('const real = 1;');
  });

  it('treats a regex containing quotes as a regex, not as code', () => {
    // `/['"]/` after `=` is a regex; reading it as code would open a string on
    // the apostrophe and eat the following statement.
    const src = 'const re = /[\'"]/;\nconst real = 1;\n';
    expect(stripCommentsAndLiterals(src)).toContain('const real = 1;');
  });

  it('handles a regex after `return`, where a naive scan sees division', () => {
    const src = 'function f(s) { return /[\'"]/.test(s); }\nconst real = 1;\n';
    expect(stripCommentsAndLiterals(src)).toContain('const real = 1;');
  });

  it('treats a slash after an operand as division, not as a regex', () => {
    const src = 'const half = total / 2;\nconst real = 1;\n';
    const out = stripCommentsAndLiterals(src);
    expect(out).toContain('total / 2');
    expect(out).toContain('const real = 1;');
  });

  it('does not run past an unterminated quote', () => {
    const src = "const bad = 'oops\nconst real = 1;\n";
    expect(stripCommentsAndLiterals(src)).toContain('const real = 1;');
  });
});

describe('containsDynamicImport', () => {
  // Must NOT be flagged — each of these is a valid classic script.
  it.each([
    ['a property named import', 'obj.import("./y.js");'],
    ['the word in a line comment', '// import("x")\nconst a = 1;'],
    ['the word in a block comment', '/* import("x") */\nconst a = 1;'],
    ['the word in a single-quoted string', 'const s = \'import("x")\';'],
    ['the word in a double-quoted string', 'const s = "import(\'x\')";'],
    ['the word in a template literal', 'const s = `import("x")`;'],
    ['an identifier that merely ends in import', 'someimport("./y.js");'],
    ['an identifier that merely starts with import', 'importantThing("./y.js");'],
    ['a $-prefixed identifier', '$import("./y.js");'],
    ['no import at all', 'const a = 1;\nfoo(2);'],
  ])('passes %s', (_label, src) => {
    expect(containsDynamicImport(src)).toBe(false);
  });

  // Must be flagged — each is a real dynamic import.
  it.each([
    ['a top-level dynamic import', 'import("./y.js");'],
    ['a guarded dynamic import', 'if (0) import("./y.js");'],
    ['an awaited dynamic import', 'async function f() { await import("./y.js"); }'],
    ['one with whitespace before the paren', 'import ("./y.js");'],
    ['one after a comment holding an apostrophe', '// don\'t\nimport("./y.js");'],
    ['one after a regex holding quotes', 'const re = /[\'"]/;\nimport("./y.js");'],
  ])('catches %s', (_label, src) => {
    expect(containsDynamicImport(src)).toBe(true);
  });
});

describe('failsToCompileAsClassicScript', () => {
  it('accepts a plain script', () => {
    expect(failsToCompileAsClassicScript('const a = 1;\nfoo(a);\n')).toBeNull();
  });

  it('accepts the browser globals an injected script uses, since nothing runs', () => {
    expect(failsToCompileAsClassicScript('document.querySelectorAll("input");\n')).toBeNull();
  });

  it('rejects a static import — the failure that actually shipped', () => {
    expect(failsToCompileAsClassicScript('import { x } from "./y.js";\n')).toMatch(
      /import statement outside a module/
    );
  });

  it('rejects an export', () => {
    expect(failsToCompileAsClassicScript('const a = 1;\nexport { a };\n')).not.toBeNull();
  });

  it('rejects a dynamic import the compile cannot see', () => {
    // Valid script syntax, so `new Script()` is happy; it just resolves nothing
    // once injected.
    expect(failsToCompileAsClassicScript('if (0) import("./y.js");\n')).toMatch(
      /dynamic import\(\)/
    );
  });

  it('does not reject a property call that merely looks like one', () => {
    expect(
      failsToCompileAsClassicScript('const o = { import: () => 1 };\no.import();\n')
    ).toBeNull();
  });
});
