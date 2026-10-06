/** "Save my answers from this page" and "Suggest answers for this form". */

import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { getClient, notPaired } from './bridge-client';
import { isCaptureResult, isScannedQuestions } from './guards';
import { activeTabUrl, readPage, requireTabId } from './page';

/** Client-side cap on the number of scanned question labels sent in one
 *  `answers.suggest` call — the desktop re-clamps independently (untrusted
 *  page-derived input), this just avoids sending an unbounded payload. */
export const MAX_SUGGEST_QUESTIONS = 50;

/**
 * User-clicked "Save my answers from this page". A deliberate click: capture
 * and transport failures propagate to the dispatcher's outer catch, and a
 * resolved desktop refusal passes through as `result`. The token check runs
 * BEFORE the capture injection, so an unpaired browser never reads the page.
 * `filled` rides the SAME capture so the popup can source its rewrite picker
 * without a second scan.
 */
export async function runAnswersSave(windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  const url = await activeTabUrl(windowId);
  const { answers, filled } = await readPage(
    await requireTabId(windowId, 'No active tab to capture.'),
    'capture.js',
    isCaptureResult,
    'Could not read the answers on this page.'
  );
  const result = await getClient().saveAnswers(url, answers);
  return { ok: true, kind: 'answersSave', result, filled };
}

/**
 * User-clicked "Suggest answers for this form". Same not-paired short-circuit
 * and never-fold-errors discipline as {@link runAnswersSave}. The scanned
 * correlation list rides alongside `result` so the popup can decide, per
 * suggestion, whether a live Fill target still exists on the page.
 */
export async function runAnswersSuggest(windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  const scanned = await readPage(
    await requireTabId(windowId, 'No active tab to scan.'),
    'capture-questions.js',
    isScannedQuestions,
    'Could not read the questions on this page.'
  );
  // Dedup by exact text (the desktop dedups by normalized text) and cap
  // client-side — untrusted page content, never send an unbounded array.
  const questions = [...new Set(scanned.map((q) => q.question))].slice(0, MAX_SUGGEST_QUESTIONS);
  const result = await getClient().suggestAnswers(questions);
  return { ok: true, kind: 'answersSuggest', result, scanned };
}
