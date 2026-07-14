/**
 * Single-field answer-fill injected entry (compiled to `answer-fill.js`).
 *
 * Injected on the user's per-row "Fill this field" click via
 * `chrome.scripting.executeScript({ files: ['answer-fill.js'] })` — NOT a
 * persistently registered content script, same `activeTab`+`scripting`
 * pattern as `fill.ts`/`capture.ts`. Two-step like `fill.ts` (not
 * `capture.ts`'s single-step): the answer TEXT is the user's own past
 * application answer — PII-adjacent — so it is passed in transiently via a
 * second `executeScript({ func, args })` rather than baked into the `files`
 * injection, exactly like `fill.ts` does for the contact profile.
 *
 * It only exposes the filler on the page's isolated-world global under
 * {@link ANSWER_FILL_GLOBAL}; the background then calls it with the
 * question/index correlation + the answer text via that second
 * `executeScript`.
 */

import { ANSWER_FILL_GLOBAL, fillAnswerField, type FillAnswerResult } from './lib/answer-fill';

(
  globalThis as unknown as Record<
    string,
    (question: string, index: number, answer: string) => FillAnswerResult
  >
)[ANSWER_FILL_GLOBAL] = (question, index, answer) =>
  fillAnswerField(document, question, index, answer);

// Ensure this file is treated as an ES module.
export {};
