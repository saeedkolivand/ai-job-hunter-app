/**
 * The shared per-(tab, origin) answer state (ADR-044): scan, free-text add,
 * version select, Accept/Restore, and the row-scoped draft/rewrite. Everything
 * is written to `storage.session`, where BOTH surfaces (popup and side panel)
 * are subscribed.
 */

import type { ExtensionRewritePreset } from '@ajh/shared';

import {
  addFreeRow,
  type AnswerRow,
  type AnswerScan,
  type AnswerState,
  buildRows,
  readAnswerState,
  rewriteBaseText,
  selectedText,
  updateAnswerState,
  writeAnswerState,
} from '../lib/answer-state';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { runAnswerAssist } from './answer-assist';
import { injectAnswerFill, injectAnswerReplace } from './answer-fill';
import { MAX_SUGGEST_QUESTIONS } from './answers';
import { getClient, notPaired } from './bridge-client';
import { isAnswerScan } from './guards';
import { activeTabId, activeTabOriginAtGesture, readPage } from './page';

/**
 * Which of `questions` a past application can already answer, and with what.
 * BEST-EFFORT: every failure (not paired, bridge down, the autofill opt-in off,
 * a desktop refusal) folds to an empty map, so a row simply stays `empty`
 * instead of the whole scan failing over a status badge. Salary-shaped
 * suggestions are dropped for the same reason the suggestion rows never offer to
 * fill them.
 */
async function savedAnswersFor(
  questions: string[]
): Promise<Map<string, { answer: string; source?: string }>> {
  const out = new Map<string, { answer: string; source?: string }>();
  if (questions.length === 0) return out;
  try {
    const result = await getClient().suggestAnswers(questions.slice(0, MAX_SUGGEST_QUESTIONS));
    if (!result.ok) return out;
    for (const s of result.suggestions) {
      if (s.salary || out.has(s.question)) continue;
      const source = [s.sourceTitle, s.sourceCompany].filter(Boolean).join(' at ');
      out.set(s.question, source ? { answer: s.answer, source } : { answer: s.answer });
    }
  } catch {
    // Folded on purpose — see this function's doc.
  }
  return out;
}

/**
 * (Re)build the shared answer state for the active tab: inject the rows
 * collector, capture the origin, look up saved answers, write the result.
 * Versions already drafted on a row survive the rescan (`buildRows`), so running
 * this on every popup open — and on the panel's Rescan for a multi-step form —
 * never costs the user work.
 */
export async function runAnswerScan(windowId?: number): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const origin = await activeTabOriginAtGesture(windowId);
  const scan: AnswerScan = await readPage(
    tabId,
    'capture-rows.js',
    isAnswerScan,
    'Could not read the questions on this page.'
  );
  const previous = await readAnswerState(tabId);
  const savedFor = await savedAnswersFor([...new Set(scan.questions.map((q) => q.question))]);

  const state: AnswerState = {
    tabId,
    origin,
    scannedAt: Date.now(),
    rows: buildRows(scan, savedFor, previous?.rows ?? []),
    stream: previous?.stream ?? null,
    // The scan itself IS the re-arm: it only ran because a gesture granted
    // `activeTab` for this tab, so whatever navigation set the flag is now
    // accounted for.
    pageChanged: false,
  };
  await writeAnswerState(state);
  return { ok: true, kind: 'answerState', state };
}

/** Add (or reuse) a free-text row — the manual entry and the context-menu
 *  selection both land here. No page access: a question the scan missed is
 *  still worth drafting, it just has nowhere to be accepted into.
 *  `explicitTabId` lets the context-menu gesture pin the row to its own tab, so
 *  a focus change between the gesture and this call can't land the row in the
 *  wrong tab's record. */
export async function runAnswerAddRow(
  question: string,
  explicitTabId?: number,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = explicitTabId ?? (await activeTabId(windowId));
  const existing = await readAnswerState(tabId);
  const state: AnswerState = existing ?? {
    tabId,
    origin: await activeTabOriginAtGesture(windowId),
    scannedAt: Date.now(),
    rows: [],
    stream: null,
    // Nothing has been scanned, so nothing claims to know the page — the write
    // controls are gated on a row HAVING a field, not on this flag.
    pageChanged: false,
  };
  const next: AnswerState = { ...state, rows: addFreeRow(state.rows, question) };
  await writeAnswerState(next);
  return { ok: true, kind: 'answerState', state: next };
}

/** Show a different version of a row. Pure state — writes nothing to the page
 *  until the user presses Accept. */
export async function runAnswerSelectVersion(
  rowId: string,
  version: number,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const state = await updateAnswerState(tabId, (current) => ({
    ...current,
    rows: current.rows.map((row) =>
      row.id === rowId
        ? { ...row, selected: version >= 0 && version < row.versions.length ? version : -1 }
        : row
    ),
  }));
  return { ok: true, kind: 'answerState', state };
}

/** Find a row by id, or throw the message the surface renders. */
function requireRow(state: AnswerState | null, rowId: string): AnswerRow {
  const row = state?.rows.find((r) => r.id === rowId);
  if (!row) throw new Error('That question is no longer on this page — rescan and try again.');
  return row;
}

/**
 * Write `text` into `row`'s field through the SAME fail-safe path the popup's
 * per-row Fill and rewrite Accept use, chosen by the row's field kind: an
 * `empty` field goes through `answer-fill.js` (refuses unless the same-question
 * EMPTY field count still matches), a `filled` one through `answer-replace.js`
 * (refuses that AND any text no longer what we believe is in the field). On
 * success an `empty` field flips to `filled` and `currentText` moves to what
 * was written, so a second Accept takes the replace path (and the rescan row
 * model rebuilds on the `filled` id, see `buildRows`).
 */
async function writeRowText(
  rowId: string,
  text: string,
  windowId?: number
): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  const tabId = await activeTabId(windowId);
  const state = await readAnswerState(tabId);
  if (state?.pageChanged) {
    return {
      ok: false,
      error: 'This page changed. Click the toolbar icon to scan it, then try again.',
    };
  }
  const row = requireRow(state, rowId);
  const field = row.field;
  if (!field) {
    return { ok: false, error: 'This question is not on the page, so there is nothing to fill.' };
  }

  const result =
    field.kind === 'empty'
      ? await injectAnswerFill(row.question, field.index, field.count, text, windowId)
      : await injectAnswerReplace(
          row.question,
          field.index,
          field.count,
          text,
          field.currentText,
          windowId
        );

  if (result.filled) {
    // A FAILED fill must not flip the kind (`result.filled` is the guard): a
    // refused write leaves the field empty, so the next Accept must still take
    // the fill path and the row model must still key on the `empty` id.
    await updateAnswerState(tabId, (current) => ({
      ...current,
      rows: current.rows.map((r) =>
        r.id === rowId && r.field
          ? { ...r, field: { ...r.field, kind: 'filled', currentText: text } }
          : r
      ),
    }));
  }
  return { ok: true, kind: 'answerAccept', result };
}

/** Accept: write the version currently on screen into the field. */
export async function runAnswerAccept(rowId: string, windowId?: number): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  return writeRowText(rowId, selectedText(row), windowId);
}

/** Restore original: put the field's FROZEN scan-time text back. */
export async function runAnswerRestoreOriginal(
  rowId: string,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  return writeRowText(rowId, row.field?.originalText ?? '', windowId);
}

/**
 * Draft or rewrite for one row, resolved against the row's OWN state so the two
 * verbs can never be confused at the call site: a rewrite starts from the row's
 * latest version (never the selected one — see `rewriteBaseText`) and carries no
 * limit; a draft carries the field's `maxlength` and no existing answer. This is
 * the single place ADR-044 decision 5's "chips reshape, Regenerate rethinks"
 * becomes two different requests.
 */
export async function runAnswerRowAssist(
  rowId: string,
  searchWeb: boolean,
  mode: 'draft' | 'rewrite',
  preset?: ExtensionRewritePreset,
  instruction?: string,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  if (mode === 'rewrite') {
    const base = rewriteBaseText(row);
    if (!base.trim()) {
      return { ok: false, error: 'There is nothing to reshape yet — draft an answer first.' };
    }
    return runAnswerAssist({
      question: row.question,
      searchWeb: false,
      mode,
      existingAnswer: base,
      preset,
      instruction,
      rowId,
      windowId,
    });
  }
  return runAnswerAssist({
    question: row.question,
    searchWeb,
    mode,
    instruction,
    rowId,
    maxChars: row.field?.maxChars,
    windowId,
  });
}
