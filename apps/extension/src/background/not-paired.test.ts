/**
 * The not-paired short-circuit: every token-gated gesture refuses BEFORE it
 * reaches the page or the bridge, so an unpaired browser never reads a tab.
 */

import { beforeEach, expect, it } from 'vitest';

import type { PopupRequest } from '../lib/messages';
import {
  executeScriptMock,
  getTokenMock,
  mockClient,
  NOT_PAIRED,
  resetMocks,
  send,
  tabsQueryMock,
} from './test-support';

beforeEach(resetMocks);

const docSource = { kind: 'document', id: 'doc-1' } as const;

it.each([
  ['fill', { kind: 'fill' }, () => [mockClient.getProfile, executeScriptMock]],
  [
    'documentExportText',
    { kind: 'documentExportText', source: docSource, templateId: 'classic' },
    () => [mockClient.documentExport],
  ],
  [
    'documentAttach',
    { kind: 'documentAttach', source: docSource, templateId: 'classic', format: 'pdf' },
    () => [mockClient.documentExport, executeScriptMock],
  ],
  ['answersSave', { kind: 'answersSave' }, () => [executeScriptMock, mockClient.saveAnswers]],
  [
    'answersSuggest',
    { kind: 'answersSuggest' },
    () => [executeScriptMock, mockClient.suggestAnswers],
  ],
  ['matchLive', { kind: 'matchLive' }, () => [executeScriptMock, mockClient.matchLive]],
  ['stampResults', { kind: 'stampResults' }, () => [executeScriptMock]],
  [
    'answerAssist',
    { kind: 'answerAssist', question: 'Why this role?', searchWeb: false },
    () => [tabsQueryMock, mockClient.answerAssist],
  ],
  [
    'answerFill',
    {
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    },
    () => [executeScriptMock],
  ],
  [
    'answerReplace',
    {
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    },
    () => [executeScriptMock],
  ],
] as [string, PopupRequest, () => { mock: { calls: unknown[] } }[]][])(
  '%s surfaces "Not paired" and never reaches the page or the bridge when no token is stored',
  async (_kind, request, untouched) => {
    getTokenMock.mockResolvedValue(null);

    expect(await send(request)).toEqual(NOT_PAIRED);
    for (const mock of untouched()) expect(mock).not.toHaveBeenCalled();
  }
);
