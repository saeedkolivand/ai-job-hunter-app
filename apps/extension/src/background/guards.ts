/**
 * Guards for values that cross the `executeScript` / `runtime.onMessage`
 * boundary — page-derived, so never trusted until shape-checked.
 *
 * Every lib import here is TYPE-ONLY, and must stay that way: `fill.js`,
 * `capture*.js`, `answer-fill.js`, `answer-replace.js`, `attach-file.js`,
 * `fit-badge.js`, `results-stamp.js` and `submit-watch.js` are classic-script
 * injection targets (see `injected-entries.mjs`), so their runtime code must be
 * imported ONLY by their own entry file. A runtime import from the background
 * would make Rollup hoist it into a shared chunk those scripts then `import`,
 * breaking injection. The tiny runtime bits the background needs (global keys,
 * message kinds) are duplicated as local literals instead, each pinned to its
 * source by a test there.
 */

import type { FillAnswerResult } from '../lib/answer-fill';
import type { AnswerScan } from '../lib/answer-state';
import type { CapturedAnswer, FilledField, ScannedQuestion } from '../lib/answers-capture';
import type { AttachFileResult } from '../lib/attach-file';
import type { AutofillSummary } from '../lib/autofill';
import type { CollectedCard } from '../lib/results-stamp';

/** Internal message kind the injected `submit-watch.js` posts on a detected
 *  form submit. MUST match `SUBMIT_DETECTED_MSG` in `lib/submit-watch.ts`
 *  (pinned by a test; exported by the entry for that). */
export const SUBMIT_DETECTED_MSG = 'submitDetected';

/** Internal message kind the fit badge's "Open the panel" button posts — MUST
 *  match `OPEN_PANEL_MSG` in `lib/fit-badge.ts`. */
const OPEN_PANEL_FROM_BADGE_MSG = 'ajhOpenPanelFromBadge';

type Shape = Record<string, 'string' | 'number' | 'boolean'>;

const isObjectOf = (v: unknown, shape: Shape): boolean =>
  typeof v === 'object' &&
  v !== null &&
  Object.entries(shape).every(([key, type]) => typeof (v as Record<string, unknown>)[key] === type);

const isArrayOf = (v: unknown, shape: Shape): boolean =>
  Array.isArray(v) && v.every((e) => isObjectOf(e, shape));

/** `{question, index}[]` — capture-questions.js's completion value. */
export const isScannedQuestions = (v: unknown): v is ScannedQuestion[] =>
  isArrayOf(v, { question: 'string', index: 'number' });

/** `{url, index}[]` — `results-stamp.js`'s collect-step return. */
export const isCollectedCards = (v: unknown): v is CollectedCard[] =>
  isArrayOf(v, { url: 'string', index: 'number' });

/** `{question, answer}[]` — capture.js's `answers` completion field. */
const isCapturedAnswers = (v: unknown): v is CapturedAnswer[] =>
  isArrayOf(v, { question: 'string', answer: 'string' });

/** `{question, index, answer}[]` — capture.js's `filled` field (the rewrite
 *  picker's source). */
const isFilledFields = (v: unknown): v is FilledField[] =>
  isArrayOf(v, { question: 'string', index: 'number', answer: 'string' });

/** answer-fill.js / answer-replace.js's completion value. */
export const isFillAnswerResult = (v: unknown): v is FillAnswerResult =>
  isObjectOf(v, { filled: 'boolean' });

/** attach-file.js's completion value. */
export const isAttachFileResult = (v: unknown): v is AttachFileResult =>
  isObjectOf(v, { attached: 'boolean' });

/** probe-fields.js's completion value — two independent booleans, not one. */
export const isFieldsProbeResult = (
  v: unknown
): v is { hasFormFields: boolean; hasAnswerFields: boolean } =>
  isObjectOf(v, { hasFormFields: 'boolean', hasAnswerFields: 'boolean' });

/** fill.js's summary. */
export function isFillSummary(v: unknown): v is AutofillSummary {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return Array.isArray(o.filled) && typeof o.filledNothing === 'boolean';
}

/** capture.js's full completion value — `{answers, filled}` (both ride the SAME
 *  injection; see `capture.ts`). */
export function isCaptureResult(
  v: unknown
): v is { answers: CapturedAnswer[]; filled: FilledField[] } {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return isCapturedAnswers(o.answers) && isFilledFields(o.filled);
}

/** capture-rows.js's completion value. */
export function isAnswerScan(v: unknown): v is AnswerScan {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return isScannedQuestions(o.questions) && isFilledFields(o.filled);
}

/** The injected submit-watcher's fire-and-forget message. `answers` is present
 *  only when it was armed with `captureAnswers: true` AND something was filled,
 *  and is validated with the SAME guard capture.js's completion value uses. */
export function isSubmitDetected(
  v: unknown
): v is { kind: 'submitDetected'; url: string; answers?: CapturedAnswer[] } {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.kind !== SUBMIT_DETECTED_MSG || typeof o.url !== 'string') return false;
  return o.answers === undefined || isCapturedAnswers(o.answers);
}

/** The fit badge's fire-and-forget "Open the panel" click. */
export function isOpenPanelFromBadge(v: unknown): v is { kind: typeof OPEN_PANEL_FROM_BADGE_MSG } {
  if (typeof v !== 'object' || v === null) return false;
  return (v as Record<string, unknown>).kind === OPEN_PANEL_FROM_BADGE_MSG;
}
