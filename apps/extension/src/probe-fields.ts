/**
 * Fillable-fields probe, injected entry (compiled to `probe-fields.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['probe-fields.js'] })`
 * when the popup's connected view opens — single-step, mirroring
 * `capture-questions.ts`'s pattern (no PII to pass in transiently; it only
 * reads the page and returns two booleans). Read-only: counts candidate
 * fields, reads no values.
 *
 * Combines TWO independent signals (deliberately different — see each
 * function's own doc):
 *  - `hasFormFields` — the UNION of `hasAutofillableFields` (identity fields
 *    Fill can use) and `hasAnswerCapturableFields` (non-identity fields Save
 *    can capture). Gates the popup's Form group ("Fill this form" / "Save my
 *    answers"): a page with ONLY name/email/phone fields still has plenty
 *    for Fill to do, even though it has nothing Save/Suggest/Rewrite can act
 *    on.
 *  - `hasAnswerFields` — `hasAnswerCapturableFields` alone. Gates the
 *    Answer-tools disclosure (Suggest/rewrite have nothing to act on without
 *    a non-identity field, so the union would be too permissive there).
 *
 * Bundled with ZERO `import` statements: `./lib/answers-capture` and
 * `./lib/autofill` are inlined here because this file is built by its OWN
 * isolated Rollup pass — see the `injectedEntries` plugin in
 * `vite.config.ts` — so it never shares a chunk with the other injected
 * entries.
 */

import { hasAnswerCapturableFields } from './lib/answers-capture';
import { hasAutofillableFields } from './lib/autofill';

// ── injected-execution entry-point ────────────────────────────────────────────
// Completion value returned to executeScript → background (mirrors
// capture-questions.ts: the IIFE's return value is the last statement's value).
(() => {
  const hasAnswerFields = hasAnswerCapturableFields(document);
  return {
    hasFormFields: hasAnswerFields || hasAutofillableFields(document),
    hasAnswerFields,
  };
})();
