/** `answersSave` ("Save my answers from this page") and `answersSuggest` — capture, then send; errors are NOT folded. */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  DESKTOP_DOWN,
  executeScriptMock,
  mockClient,
  paired,
  POSTING_URL,
  resetMocks,
  scriptResults,
  send,
  tabsQueryMock,
} from './test-support';

beforeEach(resetMocks);

const SUGGESTION = {
  question: 'Why this role?',
  answer: 'Because I love it.',
  score: 0.8,
  salary: false,
};

describe('answersSave request', () => {
  it('injects capture.js, sends the captured answers, and returns the success result', async () => {
    paired();
    const captured = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    const filled = [{ question: 'Why this role?', index: 0, answer: 'Because I love it.' }];
    scriptResults({ answers: captured, filled });
    const result = {
      ok: true,
      applicationId: 'app-1',
      saved: 1,
      skipped: 0,
      title: 'Backend Engineer',
      company: 'Acme',
    };
    mockClient.saveAnswers.mockResolvedValue(result);

    const res = await send({ kind: 'answersSave' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['capture.js'] })
    );
    expect(mockClient.saveAnswers).toHaveBeenCalledWith(POSTING_URL, captured);
    expect(res).toEqual({ ok: true, kind: 'answersSave', result, filled });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    paired('https://jobs.example.com/posting/none');
    scriptResults({ answers: [], filled: [] });
    const result = {
      ok: false,
      error: "couldn't find a saved job for this page — import it first",
    };
    mockClient.saveAnswers.mockResolvedValue(result);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: true, kind: 'answersSave', result, filled: [] });
  });

  it('surfaces "Could not read the answers on this page." when the injected script returns a non-array', async () => {
    paired();
    scriptResults(null);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: false, error: 'Could not read the answers on this page.' });
    expect(mockClient.saveAnswers).not.toHaveBeenCalled();
  });

  it('surfaces "Could not read the current tab URL." when there is no active tab, without calling saveAnswers', async () => {
    // activeTabUrl() runs BEFORE the capture injection (mirrors runStatusUpdate).
    paired();
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: false, error: 'Could not read the current tab URL.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(mockClient.saveAnswers).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false (UNLIKE appliedCheck, which folds every rejection)', async () => {
    paired();
    scriptResults({ answers: [], filled: [] });
    mockClient.saveAnswers.mockRejectedValue(new Error(DESKTOP_DOWN));

    expect(await send({ kind: 'answersSave' })).toEqual({ ok: false, error: DESKTOP_DOWN });
  });
});

describe('answersSuggest request', () => {
  it('injects capture-questions.js, sends deduped labels, and returns the success result + scanned list', async () => {
    paired();
    const scanned = [
      { question: 'Why this role?', index: 0 },
      { question: 'Why this role?', index: 0 }, // duplicate label text — deduped before send
    ];
    scriptResults(scanned);
    mockClient.suggestAnswers.mockResolvedValue({ ok: true, suggestions: [SUGGESTION] });

    const res = await send({ kind: 'answersSuggest' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['capture-questions.js'] })
    );
    expect(mockClient.suggestAnswers).toHaveBeenCalledWith(['Why this role?']);
    expect(res).toEqual({
      ok: true,
      kind: 'answersSuggest',
      result: { ok: true, suggestions: [SUGGESTION] },
      scanned,
    });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    paired();
    scriptResults([]);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'Autofill is off.' });

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({
      ok: true,
      kind: 'answersSuggest',
      result: { ok: false, error: 'Autofill is off.' },
      scanned: [],
    });
  });

  it('surfaces "Could not read the questions on this page." when the injected script returns a non-array', async () => {
    paired();
    scriptResults(null);

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({ ok: false, error: 'Could not read the questions on this page.' });
    expect(mockClient.suggestAnswers).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    paired();
    scriptResults([]);
    mockClient.suggestAnswers.mockRejectedValue(new Error(DESKTOP_DOWN));

    expect(await send({ kind: 'answersSuggest' })).toEqual({ ok: false, error: DESKTOP_DOWN });
  });
});
