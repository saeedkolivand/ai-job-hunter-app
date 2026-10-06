/**
 * Row/field builders shared by the answer-state test files. Plain builders over
 * the real `buildRows` — no mocks, so they work in any of the sibling suites.
 */

import { type AnswerRow, type AnswerScan, appendVersion, buildRows } from '../answer-state';

const NO_SAVED = new Map<string, { answer: string; source?: string }>();

export const WHY_US = { question: 'Why us?', index: 0 };
export const WHY_US_FILLED = { ...WHY_US, answer: 'A draft.' };

/** `buildRows` over a scan with the given parts and no saved answers. */
export const rowsOf = (over: Partial<AnswerScan>, previous?: AnswerRow[]): AnswerRow[] =>
  buildRows({ questions: [], filled: [], ...over }, NO_SAVED, previous);

type Field = NonNullable<AnswerRow['field']>;

export const fieldOf = (over: Partial<Field> = {}): Field => ({
  kind: 'empty',
  index: 0,
  count: 1,
  currentText: '',
  originalText: '',
  ...over,
});

export const rowOf = (over: Partial<AnswerRow> = {}): AnswerRow => ({
  id: 'r',
  question: 'Q',
  field: fieldOf(),
  status: 'empty',
  versions: [],
  selected: -1,
  ...over,
});

/** The "Why us?" row with a v1 draft on it — the starting point of the rescan tests. */
export const draftedWhyUs = (): AnswerRow[] => {
  const first = rowsOf({ questions: [WHY_US] });
  return appendVersion(first, (first[0] as AnswerRow).id, 'A draft.', 'draft');
};
