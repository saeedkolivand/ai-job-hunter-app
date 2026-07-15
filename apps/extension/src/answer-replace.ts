/**
 * Single-field answer-REPLACE injected entry (compiled to `answer-replace.js`).
 *
 * Injected on the user's "Accept"/"Restore original" click for a rewrite
 * (extension PR 11) via `chrome.scripting.executeScript({ files:
 * ['answer-replace.js'] })` — NOT a persistently registered content script,
 * same `activeTab`+`scripting` pattern as `fill.ts`/`answer-fill.ts`.
 * Two-step like `answer-fill.ts`: the replacement TEXT (the AI-rewritten
 * draft, or the frozen original answer on Restore) is passed in transiently
 * via a second `executeScript({ func, args })` rather than baked into the
 * `files` injection.
 *
 * It only exposes the replacer on the page's isolated-world global under
 * {@link ANSWER_REPLACE_GLOBAL}; the background then calls it with the
 * question/index/count correlation + the text via that second
 * `executeScript`.
 */

import {
  ANSWER_REPLACE_GLOBAL,
  type FillAnswerResult,
  replaceFilledField,
} from './lib/answer-fill';

(
  globalThis as unknown as Record<
    string,
    (
      question: string,
      index: number,
      count: number,
      text: string,
      expectedValue: string
    ) => FillAnswerResult
  >
)[ANSWER_REPLACE_GLOBAL] = (question, index, count, text, expectedValue) =>
  replaceFilledField(document, question, index, count, text, expectedValue);

// Ensure this file is treated as an ES module.
export {};
