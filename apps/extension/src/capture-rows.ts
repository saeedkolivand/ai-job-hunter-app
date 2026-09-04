/**
 * Answer-rows collector, injected entry (compiled to `capture-rows.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['capture-rows.js'] })`
 * on a user gesture (the popup's connected view, the panel's Rescan, the
 * context-menu entry) — single-step, mirroring `capture-questions.ts`. It is
 * the ONE scan behind ADR-044's per-(tab, origin) answer state: both the EMPTY
 * candidates (a question still to answer) and the FILLED ones (a question
 * already answered, whose text a rewrite starts from), each carrying its own
 * `maxlength` where the page declares one.
 *
 * Read-only, and it reuses the SAME collectors "Suggest answers"/"Save my
 * answers" already run — no new notion of "a question on this page" is
 * introduced here, so the row list can never disagree with what those two
 * features see.
 *
 * Bundled with ZERO `import` statements: `./lib/answers-capture` (and the
 * `./lib/field-signal` helpers it shares) is inlined because this file is
 * built by its OWN isolated Rollup pass — see the `injectedEntries` plugin in
 * `vite.config.mts` — so it never shares a chunk with the other injected
 * entries.
 */

import { collectFilledFields, collectQuestions } from './lib/answers-capture';

// ── injected-execution entry-point ────────────────────────────────────────────
// Completion value returned to executeScript → background (mirrors
// probe-fields.ts: a STATEMENT body, not a single-expression arrow, so a
// minifier folding the two calls into a comma expression can't drop the
// object literal around them — see vite.config.mts's `minify: false` for the
// primary fix; this is defense in depth).
(() => {
  const questions = collectQuestions(document);
  return { questions, filled: collectFilledFields(document) };
})();
