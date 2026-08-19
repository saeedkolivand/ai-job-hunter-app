import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import { ALLOWLIST, findLeaks, MIN_SITES, violations } from './check-log-error-leaks.mjs';

// Two kinds of test here, mirroring check-adr-citations.test.mjs.
//
// `findLeaks`/`violations` are exercised against a synthetic source tree and
// synthetic inventories, so a case can state exactly what it is testing
// without depending on which sites happen to be declared today. Then the
// last block runs `findLeaks()` against the REAL repo and checks it produces
// zero `violations()` against the REAL `ALLOWLIST` — a guard whose regex
// silently stopped matching the actual log-macro idiom is the one failure
// mode that makes all of the synthetic coverage theatre.
//
// `mkdtempSync` under `tmpdir()` rather than a fixed path: parallel test
// workers must not collide on the same directory.

let dir;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

/** Write `relPath` (e.g. `'foo/bar.rs'`) under a fresh temp src root; returns the root. */
function writeSrc(files) {
  dir = mkdtempSync(join(tmpdir(), 'check-log-error-leaks-'));
  for (const [rel, content] of Object.entries(files)) {
    const full = join(dir, rel);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, content);
  }
  return dir;
}

describe('findLeaks', () => {
  it('flags a single-line captured `{e}` interpolation', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {e}");\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {e}' },
    ]);
  });

  it('finds the site on the LINE the literal `{e}` sits on, not the macro line', () => {
    // A backslash-continued string literal (the real shape this codebase used
    // for a long message, e.g. dedup/mod.rs before it was fixed) — `{e}` sits
    // one line below the `log::warn!(` call itself.
    const root = writeSrc({
      'foo.rs': [
        'log::warn!(',
        '    "[foo] failed ({e}); degrading — \\',
        '     more text on a continuation line"',
        ');',
        '',
      ].join('\n'),
    });
    expect(findLeaks(root)).toEqual([
      {
        key: 'foo.rs:2',
        file: 'foo.rs',
        line: 2,
        // The whitespace-collapsed literal, so a re-wrap of the same message
        // onto different line lengths still reads as the same signature.
        sig: '[foo] failed ({e}); degrading — \\ more text on a continuation line',
      },
    ]);
  });

  it('does not flag `.code()`', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", e.code());\n' });
    expect(findLeaks(root)).toEqual([]);
  });

  it('does not flag `.kind()`', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", e.kind());\n' });
    expect(findLeaks(root)).toEqual([]);
  });

  it('does not flag sanitize_reason(&e.to_string())', () => {
    const root = writeSrc({
      'foo.rs': 'log::warn!("[foo] failed: {}", sanitize_reason(&e.to_string()));\n',
    });
    expect(findLeaks(root)).toEqual([]);
  });

  it('flags a positional `e.to_string()` — the method-call gap a review found', () => {
    // The finding: the bare-identifier check missed `e.to_string()`, one
    // method call away from the exact same leak as a bare `e`.
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", e.to_string());\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {}' },
    ]);
  });

  it('flags a positional `err.to_owned()`, `&`-prefixed, same shape as `.to_string()`', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", &err.to_owned());\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {}' },
    ]);
  });

  it('flags `error.to_string()` too, for the same reason as `e`/`err`', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", error.to_string());\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {}' },
    ]);
  });

  it('still does not flag sanitize_reason(&e.to_string()) once .to_string() is matched', () => {
    // The false-positive risk this widening introduces: sanitize_reason's own
    // wrapper CONTAINS the literal text `.to_string()` inside it. Excluded by
    // the wrapping call (sanitize_reason(...) is not itself a bare
    // `e.to_string()` argument), never by pattern-matching the inner text —
    // see findPositionalErrorArg's doc comment.
    const root = writeSrc({
      'foo.rs': 'log::warn!("[foo] failed: {}", sanitize_reason(&e.to_string()));\n',
    });
    expect(findLeaks(root)).toEqual([]);
  });

  it('does not flag `.to_string()` on a non-error-binding identifier', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] value: {}", status.to_string());\n' });
    expect(findLeaks(root)).toEqual([]);
  });

  it('flags a positional bare `e` argument — the vacuity hole review found live', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {}", e);\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {}' },
    ]);
  });

  it('flags a positional bare `err` argument, `&`-prefixed', () => {
    // The exact shape live at autopilot_helpers/mod.rs:151 before this fix.
    const root = writeSrc({
      'foo.rs': "log::warn!(\"[foo] board '{}' failed (error='{}')\", board, &err);\n",
    });
    expect(findLeaks(root)).toEqual([
      {
        key: 'foo.rs:1',
        file: 'foo.rs',
        line: 1,
        sig: "[foo] board '{}' failed (error='{}')",
      },
    ]);
  });

  it('does not flag a positional argument named something other than e/err/error', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] status: {}", status);\n' });
    expect(findLeaks(root)).toEqual([]);
  });

  it('flags `{err}` — a captured identifier this scanner used to miss entirely', () => {
    // Live (before this fix) at postings/mod.rs:382 and
    // platform/linux_appimage.rs:170.
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {err}");\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {err}' },
    ]);
  });

  it('flags `{error}` too, for the same reason as `{err}`', () => {
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] failed: {error}");\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'foo.rs:1', file: 'foo.rs', line: 1, sig: '[foo] failed: {error}' },
    ]);
  });

  it('does not flag a captured identifier that is not an error-binding name', () => {
    // Bounded matching, not "any identifier" — see ERROR_BINDING_NAMES. `host`
    // and `port` are real, frequent captures in this crate's log calls, and
    // neither is an error.
    const root = writeSrc({ 'foo.rs': 'log::warn!("[foo] connecting to {host}:{port}");\n' });
    expect(findLeaks(root)).toEqual([]);
  });

  it('catches every log level', () => {
    const root = writeSrc({
      'foo.rs': [
        'log::warn!("a: {e}");',
        'log::error!("b: {e}");',
        'log::info!("c: {e}");',
        'log::debug!("d: {e}");',
        '',
      ].join('\n'),
    });
    expect(findLeaks(root).map((l) => l.key)).toEqual([
      'foo.rs:1',
      'foo.rs:2',
      'foo.rs:3',
      'foo.rs:4',
    ]);
  });

  it('scans nested directories and produces POSIX-separated keys', () => {
    const root = writeSrc({ 'a/b/c.rs': 'log::warn!("nested: {e}");\n' });
    expect(findLeaks(root)).toEqual([
      { key: 'a/b/c.rs:1', file: 'a/b/c.rs', line: 1, sig: 'nested: {e}' },
    ]);
  });
});

describe('violations', () => {
  const leak = (key, sig) => {
    const [file, line] = [key.slice(0, key.lastIndexOf(':')), Number(key.split(':').pop())];
    return { key, file, line, sig };
  };

  it('passes when every finding is declared and every entry still has a finding', () => {
    const inv = { 'foo.rs:1': { status: 'safe', reason: 'a'.repeat(30) } };
    expect(violations(inv, [leak('foo.rs:1')])).toEqual([]);
  });

  it('fails on an undeclared finding — the case this guard exists for', () => {
    const problems = violations({}, [leak('foo.rs:1')]);
    expect(problems.join('\n')).toContain('foo.rs:1');
    expect(problems.join('\n')).toContain('not declared in ALLOWLIST');
  });

  it('fails on a stale entry whose site no longer exists — keeps the list from rotting', () => {
    const inv = { 'foo.rs:1': { status: 'safe', reason: 'a'.repeat(30) } };
    const problems = violations(inv, []);
    expect(problems.join('\n')).toContain('foo.rs:1');
    expect(problems.join('\n')).toContain('Declared in ALLOWLIST but no `{e}` site found');
  });

  it('fails on a reason that is too short to be an explanation', () => {
    const inv = { 'foo.rs:1': { status: 'safe', reason: 'because' } };
    const problems = violations(inv, [leak('foo.rs:1')]);
    expect(problems.join('\n')).toContain('must state why');
  });

  it('fails on an invalid status', () => {
    const inv = { 'foo.rs:1': { status: 'maybe', reason: 'a'.repeat(30) } };
    const problems = violations(inv, [leak('foo.rs:1')]);
    expect(problems.join('\n')).toContain("must be 'safe' or 'debt'");
  });

  it('a single stale entry among many is reported as stale, not as broken detection', () => {
    // The exact case an inventory-size-relative vacuity check would get wrong:
    // 56 real findings for 57 declared entries must read as "one went stale",
    // not "detection is broken" — this is the normal, common case (one site
    // got fixed and its entry wasn't removed yet).
    const inv = Object.fromEntries(
      Array.from({ length: 57 }, (_, i) => [
        `foo.rs:${i}`,
        { status: 'safe', reason: 'x'.repeat(30) },
      ])
    );
    const leaks = Array.from({ length: 56 }, (_, i) => leak(`foo.rs:${i}`));
    const problems = violations(inv, leaks);
    expect(problems.join('\n')).toContain('foo.rs:56');
    expect(problems.join('\n')).not.toContain('detection looks');
  });

  // ── `sig`: content-anchored matching survives a line-number shift ───────
  //
  // The defect a 2026-08-19 review found: the ALLOWLIST's only identity is
  // `"<path>:<line>"`, so an unrelated edit that inserts a line above a
  // declared site breaks the guard BOTH ways at once (undeclared at the new
  // line, stale at the old one).

  it('an entry with a sig survives its site moving to a new line', () => {
    const inv = { 'foo.rs:10': { status: 'safe', reason: 'a'.repeat(30), sig: 'boom: {e}' } };
    // The site is now at line 33 (an unrelated insertion above it), same text.
    expect(violations(inv, [leak('foo.rs:33', 'boom: {e}')])).toEqual([]);
  });

  it("without a sig, the same line shift still breaks both ways (today's bar)", () => {
    const inv = { 'foo.rs:10': { status: 'safe', reason: 'a'.repeat(30) } };
    const problems = violations(inv, [leak('foo.rs:33', 'boom: {e}')]);
    expect(problems.join('\n')).toContain('foo.rs:33');
    expect(problems.join('\n')).toContain('not declared in ALLOWLIST');
    expect(problems.join('\n')).toContain('foo.rs:10');
    expect(problems.join('\n')).toContain('Declared in ALLOWLIST but no `{e}` site found');
  });

  it('a sig match is scoped to the same file — a same-text site in another file is still undeclared', () => {
    const inv = { 'foo.rs:10': { status: 'safe', reason: 'a'.repeat(30), sig: 'boom: {e}' } };
    const problems = violations(inv, [leak('bar.rs:10', 'boom: {e}')]);
    expect(problems.join('\n')).toContain('bar.rs:10');
    expect(problems.join('\n')).toContain('not declared in ALLOWLIST');
  });

  it('an AMBIGUOUS sig (two real sites, one text) falls back to line-exact rules for both', () => {
    // Deliberately conservative: a `sig` only covers an UNAMBIGUOUS pairing.
    // Two genuinely distinct call sites sharing the same message text must
    // not let one declaration silently cover both.
    const inv = { 'foo.rs:10': { status: 'safe', reason: 'a'.repeat(30), sig: 'boom: {e}' } };
    const problems = violations(inv, [
      leak('foo.rs:20', 'boom: {e}'),
      leak('foo.rs:40', 'boom: {e}'),
    ]);
    expect(problems.join('\n')).toContain('foo.rs:20');
    expect(problems.join('\n')).toContain('foo.rs:40');
    expect(problems.join('\n')).toContain('not declared in ALLOWLIST');
    expect(problems.join('\n')).toContain('foo.rs:10');
    expect(problems.join('\n')).toContain('Declared in ALLOWLIST but no `{e}` site found');
  });
});

describe('MIN_SITES (the CLI-level absolute floor, not folded into violations())', () => {
  it('is well below the real repo count, so a real regression is what trips it', () => {
    expect(findLeaks().length).toBeGreaterThan(MIN_SITES);
  });
});

describe('the real ALLOWLIST against the real repo', () => {
  it('has zero violations — the guard is green against actual source today', () => {
    const leaks = findLeaks();
    expect(violations(ALLOWLIST, leaks)).toEqual([]);
  });
});
