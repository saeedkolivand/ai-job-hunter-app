/**
 * The BUILT injected scripts still hand `executeScript` the completion value
 * the background parses.
 *
 * ## Why this exists
 *
 * `chrome.scripting.executeScript({ files: ['capture.js'] })` returns whatever
 * the injected file's LAST STATEMENT evaluates to. That completion value is a
 * wire contract — `background.ts`'s `isCaptureResult` / `isScannedQuestions` /
 * `isFieldsProbeResult` reject anything else — and it is a contract expressed
 * ONLY as "the value of a trailing expression", which is exactly the thing a
 * JS minifier is allowed to rewrite when it decides nobody reads the value.
 *
 * Vite 8's default minifier (oxc) did: it folded `capture.ts`'s
 * `(() => ({ answers: a(document), filled: b(document) }))()` into
 * `a(document),b(document);`, so the completion value became the last CALL's
 * array. `isCaptureResult` rejected it and "Save my answers from this page"
 * answered "Could not read the answers on this page." on every page — in the
 * store build, in the release zip, and in a fresh local build. Every unit test
 * stayed green, because every one of them imports the MODULE and calls the
 * exported collectors; not one of them ran the file the browser runs.
 *
 * So this test runs the artifact. It builds each completion-value entry with
 * the SHIPPING options (`injectedEntryConfig` from `vite.config.mts` — not a
 * hand-written copy, and not a possibly-stale `dist/`) into a temp dir,
 * evaluates the built file in a jsdom realm the way an injected classic script
 * is evaluated, and asserts the completion value `vm.runInContext` returns.
 * Re-enable minification for that pass (`minify: true` in `injectedEntryConfig`)
 * and the first assertion of the capture case — the completion value is not an
 * array — goes red, instead of the store build.
 *
 * Only the four entries that COMMUNICATE by completion value are covered here.
 * `fill` / `answer-fill` / `answer-replace` / `submit-watch` install a global
 * instead and are already covered by their own module tests.
 */

import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runInContext } from 'node:vm';

import { JSDOM } from 'jsdom';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { INJECTED_ENTRIES, injectedEntryConfig } from '../vite.config.mts';

/** The injected entries whose completion value the background parses. */
const COMPLETION_VALUE_ENTRIES = ['capture', 'capture-questions', 'content', 'probe-fields'];

/**
 * An application form as the collectors see one: two labelled textareas the
 * user has ANSWERED (what `capture` must return) and one labelled, still-EMPTY
 * input (what `capture-questions` must return). Both halves in one fixture, so
 * a script that returns the wrong half of its own work — the exact shape of the
 * minifier bug — cannot pass by returning a non-empty array of the other kind.
 */
const FIXTURE = `
  <form>
    <label for="q1">Why do you want to work at CUBE?</label>
    <textarea id="q1">Because the problem is the kind I like.</textarea>
    <label for="q2">What motivates you about this role?</label>
    <textarea id="q2">Shipping things people actually use.</textarea>
    <label for="q3">Notice period?</label>
    <input id="q3" type="text" value="" />
  </form>
`;

let outDir = '';
const built = new Map<string, string>();

/** The built file's text, as `executeScript` would inject it. */
function source(entry: string): string {
  const code = built.get(entry);
  if (code === undefined) throw new Error(`${entry}.js was not built`);
  return code;
}

/**
 * Evaluate `code` as a classic script against a fresh document and return its
 * COMPLETION VALUE — the same thing `executeScript({ files })` reports back.
 */
function runInPage(code: string): unknown {
  const dom = new JSDOM(`<!doctype html><html><body>${FIXTURE}</body></html>`, {
    runScripts: 'outside-only',
  });
  return runInContext(code, dom.getInternalVMContext());
}

beforeAll(async () => {
  const { build } = await import('vite');
  outDir = mkdtempSync(join(tmpdir(), 'ajh-ext-build-'));
  for (const entry of COMPLETION_VALUE_ENTRIES) {
    await build(injectedEntryConfig(entry, outDir));
    built.set(entry, readFileSync(join(outDir, `${entry}.js`), 'utf8'));
  }
}, 180_000);

afterAll(() => {
  if (outDir) rmSync(outDir, { recursive: true, force: true });
});

describe('built injected scripts — completion values', () => {
  it('capture.js returns the {answers, filled} object background.ts requires', () => {
    const result = runInPage(source('capture'));

    // The regression itself: the minifier left the two CALLS and dropped the
    // object literal around them, so the completion value was `filled` alone.
    expect(Array.isArray(result)).toBe(false);
    expect(result).toEqual({
      answers: [
        {
          question: 'Why do you want to work at CUBE?',
          answer: 'Because the problem is the kind I like.',
        },
        {
          question: 'What motivates you about this role?',
          answer: 'Shipping things people actually use.',
        },
      ],
      filled: [
        {
          question: 'Why do you want to work at CUBE?',
          index: 0,
          answer: 'Because the problem is the kind I like.',
        },
        {
          question: 'What motivates you about this role?',
          index: 0,
          answer: 'Shipping things people actually use.',
        },
      ],
    });
  });

  it('capture-questions.js returns the {question, index}[] of the EMPTY fields', () => {
    const result = runInPage(source('capture-questions'));

    expect(result).toEqual([{ question: 'Notice period?', index: 0 }]);
  });

  it('content.js returns the serialised document', () => {
    const result = runInPage(source('content'));

    expect(typeof result).toBe('string');
    expect(result as string).toMatch(/^<html/);
    expect(result as string).toContain('Why do you want to work at CUBE?');
  });

  it('probe-fields.js returns the {hasFormFields, hasAnswerFields} probe object', () => {
    const result = runInPage(source('probe-fields'));

    expect(result).toEqual({ hasFormFields: true, hasAnswerFields: true });
  });

  // This file builds its own copies, so it cannot notice an entry silently
  // dropped from the plugin's loop — the artifact would stop being emitted
  // while every assertion above still passed. Pin the membership instead.
  it('each covered entry is one the extension build actually emits', () => {
    for (const entry of COMPLETION_VALUE_ENTRIES) {
      expect(INJECTED_ENTRIES).toContain(entry);
    }
  });
});
