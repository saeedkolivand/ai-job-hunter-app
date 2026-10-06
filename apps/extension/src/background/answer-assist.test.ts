/** `answerAssist` — the first billable-AI verb; errors are NOT folded. */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  activeTab,
  assistOk,
  DESKTOP_DOWN,
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  paired,
  POSTING_URL,
  resetMocks,
  send,
} from './test-support';

beforeEach(resetMocks);

const QUESTION = 'Why this role?';
const ask = (extra: Record<string, unknown> = {}) =>
  send({ kind: 'answerAssist', question: QUESTION, searchWeb: false, ...extra } as never);
/** What the desktop was asked — the payload plus the streaming callback. */
const expectPayload = (payload: unknown) =>
  expect(mockClient.answerAssist).toHaveBeenCalledWith(payload, expect.any(Function));

describe('answerAssist request', () => {
  beforeEach(() => {
    paired();
    mockClient.answerAssist.mockResolvedValue(assistOk('Because…'));
  });

  it('sends { question, url, searchWeb } and returns the success result', async () => {
    mockClient.answerAssist.mockResolvedValue({
      ...assistOk('Because…'),
      sourced: { brief: true },
    });

    const res = await ask({ searchWeb: true });

    expectPayload({ question: QUESTION, searchWeb: true, url: POSTING_URL });
    expect(res).toEqual({
      ok: true,
      kind: 'answerAssist',
      result: { ok: true, question: QUESTION, draft: 'Because…', sourced: { brief: true } },
    });
  });

  it('still sends the request without a url when the active tab url cannot be read (generic grounding)', async () => {
    activeTab('');

    await ask();

    expectPayload({ question: QUESTION, searchWeb: false });
  });

  it('forwards mode/existingAnswer/preset/instruction for a rewrite request (PR 11)', async () => {
    const existingAnswer = 'Because I really love it and want to work here.';

    await ask({ mode: 'rewrite', existingAnswer, preset: 'shorten' });

    expectPayload({
      question: QUESTION,
      searchWeb: false,
      url: POSTING_URL,
      mode: 'rewrite',
      existingAnswer,
      preset: 'shorten',
    });
  });

  it('forwards a free-text instruction instead of a preset', async () => {
    await ask({
      mode: 'rewrite',
      existingAnswer: 'Because I like it.',
      instruction: 'Make this sound more confident.',
    });

    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'rewrite', instruction: 'Make this sound more confident.' }),
      expect.any(Function)
    );
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    mockClient.answerAssist.mockResolvedValue({ ok: false, error: 'AI answer drafting is off.' });

    expect(await ask()).toEqual({
      ok: true,
      kind: 'answerAssist',
      result: { ok: false, error: 'AI answer drafting is off.' },
    });
  });

  it('forwards the Prep tab topic field (PR4) — no rowId, so the no-row branch runs', async () => {
    const result = assistOk('Acme makes widgets.', 'Company brief');
    mockClient.answerAssist.mockResolvedValue(result);

    const res = await send({
      kind: 'answerAssist',
      question: 'Company brief',
      searchWeb: false,
      topic: 'company-brief',
    });

    expectPayload({
      question: 'Company brief',
      searchWeb: false,
      url: POSTING_URL,
      topic: 'company-brief',
    });
    expect(res).toEqual({ ok: true, kind: 'answerAssist', result });
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    mockClient.answerAssist.mockRejectedValue(new Error(DESKTOP_DOWN));

    expect(await ask()).toEqual({ ok: false, error: DESKTOP_DOWN });
  });
});

describe('answerAssist — mid-run supersession before the request is sent', () => {
  // Narrower variant of the stream race (see answer-assist-stream.test.ts): run
  // A is held BEFORE it ever resets the buffer (its own `getToken()` await still
  // pending) while run B starts AND fully completes a whole round trip. When A's
  // await finally resolves, A must recognize it has been superseded and bail out
  // WITHOUT resetting the buffer B just finished and WITHOUT ever calling the
  // billable streaming client a second time.
  it('a run superseded before its own reset never resets the buffer or calls the streaming client', async () => {
    activeTab();
    let resolveTokenA: ((value: string) => void) | undefined;
    const pendingTokenA = new Promise<string>((resolve) => {
      resolveTokenA = resolve;
    });
    getTokenMock.mockReturnValueOnce(pendingTokenA); // run A's getToken()
    getTokenMock.mockResolvedValue(FAKE_TOKEN); // run B's (and any later) getToken()

    // Start run A but don't await it — its getToken() await stays pending.
    const runA = ask({ question: 'Q1' });

    // Run B starts and fully completes while A is still stuck before its reset.
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('B chunk');
        return assistOk('B chunk', 'Q2');
      }
    );
    await ask({ question: 'Q2' });

    const settled = {
      ok: true,
      kind: 'answerAssistProgress',
      text: 'B chunk',
      done: true,
      interrupted: false,
      rowId: '',
    };
    expect(await send({ kind: 'answerAssistProgress' })).toEqual(settled);

    // A's getToken() finally resolves — A must bail out as superseded before
    // resetting the buffer, and must never call the streaming client again.
    resolveTokenA?.(FAKE_TOKEN);
    const resA = await runA;

    expect(resA).toEqual({ ok: false, error: 'Superseded by a newer request.' });
    expect(mockClient.answerAssist).toHaveBeenCalledTimes(1); // only B's call
    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      expect.objectContaining({ question: 'Q2' }),
      expect.any(Function)
    );
    expect(await send({ kind: 'answerAssistProgress' })).toEqual(settled);
  });
});
