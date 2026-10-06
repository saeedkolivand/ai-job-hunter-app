/**
 * Row + state builders shared by the answer-tools suites (`decisions.test.ts`
 * and `answer-tools.test.ts`).
 */

import type { AnswerRow, AnswerState } from '../lib/answer-state';

export const row = (over: Partial<AnswerRow> = {}): AnswerRow => ({
  id: 'r',
  question: 'Why do you want to work here?',
  field: { kind: 'empty', index: 0, count: 1, currentText: '', originalText: '' },
  status: 'empty',
  versions: [],
  selected: -1,
  ...over,
});

export const stateOf = (over: Partial<AnswerState> = {}): AnswerState => ({
  tabId: 1,
  origin: 'https://example.com',
  scannedAt: 0,
  stream: null,
  pageChanged: false,
  rows: [],
  ...over,
});
