/**
 * Questions-mode collector injected entry (compiled to `capture-questions.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['capture-questions.js']
 * })` on the user's "Suggest answers for this form" click — single-step,
 * mirroring `capture.ts`'s pattern (the collector takes no PII to pass in
 * transiently, it only reads the page and returns data).
 *
 * Bundled with ZERO `import` statements: `./lib/answers-capture` (and the
 * `./lib/field-signal` helpers it shares with `autofill.ts`/`capture.ts`) is
 * inlined here because this file is built by its OWN isolated Rollup pass —
 * see the `injectedEntries` plugin in `vite.config.ts` — so it never shares a
 * chunk with `fill.js`/`capture.js`/`answer-fill.js`.
 */

import { collectQuestions } from './lib/answers-capture';

// ── injected-execution entry-point ────────────────────────────────────────────
// Completion value returned to executeScript → background (mirrors
// capture.ts: the IIFE's return value is the last statement's value).
(() => collectQuestions(document))();
