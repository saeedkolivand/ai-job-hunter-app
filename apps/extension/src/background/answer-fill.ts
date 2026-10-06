/**
 * Single-field write-backs: fill an empty field, or replace a filled one. Both
 * fail safe (`{filled:false, error}`) on any page mutation since the scan, and
 * NEVER write to a different field than the one that was scanned/picked.
 */

import type { FillAnswerResult } from '../lib/answer-fill';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { notPaired } from './bridge-client';
import { isFillAnswerResult } from './guards';
import { injectAndRun, requireTabId } from './page';

/** Isolated-world global keys under which `answer-fill.js` / `answer-replace.js`
 *  expose their runners. MUST match `ANSWER_FILL_GLOBAL` / `ANSWER_REPLACE_GLOBAL`
 *  in `lib/answer-fill.ts` (pinned by a test there). */
const ANSWER_FILL_GLOBAL = '__ajhRunAnswerFill';
const ANSWER_REPLACE_GLOBAL = '__ajhRunAnswerReplace';

/**
 * Inject the single-field filler and run it against `(question, index)` —
 * refusing unless the CURRENT count of same-question fields still equals
 * scan-time `count` — with `answer` (the user's own past answer, passed in
 * transiently rather than baked into the `files` injection).
 */
export async function injectAnswerFill(
  question: string,
  index: number,
  count: number,
  answer: string,
  windowId?: number
): Promise<FillAnswerResult> {
  const tabId = await requireTabId(windowId, 'No active tab to fill.');
  const result = await injectAndRun(tabId, 'answer-fill.js', ANSWER_FILL_GLOBAL, [
    question,
    index,
    count,
    answer,
  ]);
  if (!isFillAnswerResult(result)) throw new Error('Could not fill this field.');
  return result;
}

/**
 * Inject the single-field REPLACER and run it against `(question, index)` —
 * refusing unless the CURRENT count of same-question FILLED fields still equals
 * pick-time `count`, AND unless the field's CURRENT text still equals
 * `expectedValue` (never overwrite a manual edit made since the pick) — with
 * `text` (the AI-rewritten draft, or the frozen original on Restore).
 */
export async function injectAnswerReplace(
  question: string,
  index: number,
  count: number,
  text: string,
  expectedValue: string,
  windowId?: number
): Promise<FillAnswerResult> {
  const tabId = await requireTabId(windowId, 'No active tab to fill.');
  const result = await injectAndRun(tabId, 'answer-replace.js', ANSWER_REPLACE_GLOBAL, [
    question,
    index,
    count,
    text,
    expectedValue,
  ]);
  if (!isFillAnswerResult(result)) throw new Error('Could not replace this field.');
  return result;
}

/** Per-row "Fill this field" click. A deliberate click: failures are NOT folded away. */
export async function runAnswerFill(
  question: string,
  index: number,
  count: number,
  answer: string,
  windowId?: number
): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();
  const result = await injectAnswerFill(question, index, count, answer, windowId);
  return { ok: true, kind: 'answerFill', result };
}

/** Rewrite mode's Accept/Restore click — never submits the form. */
export async function runAnswerReplace(
  question: string,
  index: number,
  count: number,
  text: string,
  expectedValue: string,
  windowId?: number
): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();
  const result = await injectAnswerReplace(question, index, count, text, expectedValue, windowId);
  return { ok: true, kind: 'answerReplace', result };
}
