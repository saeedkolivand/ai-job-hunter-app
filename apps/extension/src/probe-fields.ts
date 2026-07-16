/**
 * Fillable-fields probe, injected entry (compiled to `probe-fields.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['probe-fields.js'] })`
 * when the popup's connected view opens — single-step, mirroring
 * `capture-questions.ts`'s pattern (no PII to pass in transiently; it only
 * reads the page and returns one boolean). Read-only: counts candidate
 * fields, reads no values.
 *
 * Bundled with ZERO `import` statements: `./lib/answers-capture` is inlined
 * here because this file is built by its OWN isolated Rollup pass — see the
 * `injectedEntries` plugin in `vite.config.ts` — so it never shares a chunk
 * with the other injected entries.
 */

import { hasFillableFields } from './lib/answers-capture';

// ── injected-execution entry-point ────────────────────────────────────────────
// Completion value returned to executeScript → background (mirrors
// capture-questions.ts: the IIFE's return value is the last statement's value).
(() => hasFillableFields(document))();
