/**
 * Leaf DOM builders for the Documents tab — the résumé/cover-letter toggle and
 * the cover-letter paste-target picker. Pure functions of their arguments; all
 * state lives in `documents.ts`, which passes callbacks back in.
 */

import type { AnswerState } from '../lib/answer-state';
import { button, el } from '../lib/dom';

export type DocumentKind = 'resume' | 'cover-letter';

export function buildKindRow(
  kind: DocumentKind,
  hasCoverLetter: boolean,
  setKind: (kind: DocumentKind) => void
): HTMLElement {
  const row = el('div', 'kind-row');
  for (const [value, label] of [
    ['resume', 'Résumé'],
    ['cover-letter', 'Cover letter'],
  ] as const) {
    const btn = button('btn btn--small', label);
    btn.setAttribute('aria-pressed', String(kind === value));
    if (kind === value) btn.classList.add('btn--primary');
    if (value === 'cover-letter') {
      btn.disabled = !hasCoverLetter;
      btn.title = hasCoverLetter
        ? ''
        : 'This source has no cover letter — generate one in the app first.';
    }
    btn.addEventListener('click', () => setKind(value));
    row.append(btn);
  }
  return row;
}

/** The "Paste into…" list: one button per row with a live `.field` (none once
 *  the page has changed — the same trust signal every write control gates on). */
export function buildPastePicker(
  state: AnswerState | null,
  busy: boolean,
  onPick: (rowId: string) => void,
  onCancel: () => void
): HTMLElement {
  const wrap = el('div', 'picker');
  wrap.append(el('p', 'field-label', 'Paste into…'));
  const rows = (state?.rows ?? []).filter((r) => r.field !== null && !state?.pageChanged);
  if (rows.length === 0) {
    wrap.append(el('p', 'msg msg--muted', 'No form fields found on this page to paste into.'));
    return wrap;
  }
  for (const row of rows) {
    const rowBtn = button('btn btn--small btn--quiet picker__row', row.question);
    rowBtn.disabled = busy;
    rowBtn.addEventListener('click', () => onPick(row.id));
    wrap.append(rowBtn);
  }
  const cancel = button('btn btn--small btn--quiet', 'Cancel');
  cancel.addEventListener('click', onCancel);
  wrap.append(cancel);
  return wrap;
}
