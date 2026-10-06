/**
 * `answerAssist` streaming buffer — the background OWNS the accumulation so a
 * popup that closes mid-stream and reopens can reattach.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { PopupResponse } from '../lib/messages';
import { assistOk, browser, mockClient, paired, resetMocks, send } from './test-support';

beforeEach(async () => {
  await resetMocks();
  paired();
});

type OnChunk = ((d: string) => void) | undefined;

const ask = (question: string) => send({ kind: 'answerAssist', question, searchWeb: false });
const progress = (text: string, done: boolean, interrupted: boolean, rowId = '') => ({
  ok: true,
  kind: 'answerAssistProgress',
  text,
  done,
  interrupted,
  rowId,
});
const bufferNow = () => send({ kind: 'answerAssistProgress' });

describe('answerAssist streaming buffer', () => {
  it('accumulates onChunk deltas, broadcasts progress, and answerAssistProgress reflects the final done state', async () => {
    const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
    sendMessageMock.mockClear();
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: OnChunk) => {
      onChunk?.('Because I ');
      onChunk?.('am drawn to it.');
      return assistOk('Because I am drawn to it.');
    });

    await ask('Why this role?');

    // At least one live progress push happened per chunk (best-effort, so we
    // only assert the FINAL broadcast carried the fully-accumulated text).
    const pushes = sendMessageMock.mock.calls
      .map((call) => call[0] as PopupResponse)
      .filter((m) => m.ok && m.kind === 'answerAssistProgress');
    expect(pushes.length).toBeGreaterThan(0);
    expect(pushes.at(-1)).toEqual(progress('Because I am drawn to it.', true, false));
    expect(await bufferNow()).toEqual(progress('Because I am drawn to it.', true, false));
  });

  it('marks the buffer interrupted when the stream fails after some text already accumulated', async () => {
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: OnChunk) => {
      onChunk?.('Because I ');
      throw new Error('Connection to the desktop app closed.');
    });

    await expect(ask('Why this role?')).resolves.toEqual({
      ok: false,
      error: 'Connection to the desktop app closed.',
    });

    expect(await bufferNow()).toEqual(progress('Because I ', true, true));
  });

  it('a fresh answerAssist call resets the buffer, even after a prior interrupted stream', async () => {
    mockClient.answerAssist.mockImplementationOnce(async (_payload, onChunk?: OnChunk) => {
      onChunk?.('stale partial text');
      throw new Error('boom');
    });
    await ask('Q1');
    expect(await bufferNow()).toMatchObject({ text: 'stale partial text', interrupted: true });

    // A NEW call must reset the buffer — the stale interrupted text/flag from
    // the prior request must never leak into this one, even before the first
    // chunk of the new stream arrives.
    mockClient.answerAssist.mockImplementationOnce(async (_payload, onChunk?: OnChunk) => {
      expect(await bufferNow()).toEqual(progress('', false, false));
      onChunk?.('fresh answer');
      return assistOk('fresh answer', 'Q2');
    });
    await ask('Q2');
  });

  it('caps assistBuffer growth at 4000 chars even across many chunks, so the interrupted path never shows unbounded text', async () => {
    const bigChunk = 'x'.repeat(3_000);
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: OnChunk) => {
      onChunk?.(bigChunk); // 3,000
      onChunk?.(bigChunk); // 6,000 — over the 4,000 cap
      throw new Error('stream interrupted');
    });

    await ask('Why this role?');

    const buffer = (await bufferNow()) as { text: string; done: boolean; interrupted: boolean };
    expect(buffer.done).toBe(true);
    expect(buffer.interrupted).toBe(true);
    expect(buffer.text.length).toBe(4_000);
  });

  // Reachable in production: MV3 tears down the popup on close, and
  // `reattachAssistProgress` re-renders an in-flight stream without re-disabling
  // `btnAssist` — closing the popup mid-stream and reopening it lets the user
  // re-click "Help me answer…" while the first call is still in flight. Without
  // the `assistGeneration` single-flight guard, run A's late chunk and terminal
  // write clobber run B's buffer once A settles.
  it("a superseded run's late chunk and terminal write never corrupt a newer overlapping run's buffer", async () => {
    let chunkA: OnChunk;
    let resolveA: ((value: unknown) => void) | undefined;
    const pendingA = new Promise((resolve) => {
      resolveA = resolve;
    });
    mockClient.answerAssist.mockImplementationOnce(async (_payload, onChunk?: OnChunk) => {
      chunkA = onChunk;
      return pendingA;
    });

    // Start run A (a stream left running when the popup closed) but don't await
    // it. Flush a macrotask (not just a microtask) so A's OWN setup awaits
    // (getToken + activeTabUrl) fully resolve and it reaches the actual
    // streaming call (registering chunkA) BEFORE run B ever starts — otherwise
    // B's synchronous generation bump would supersede A during its own setup,
    // which is a different case (covered in answer-assist.test.ts).
    const runA = ask('Q1');
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Run B (the reopened popup's re-click) starts and fully completes while A
    // is still pending.
    mockClient.answerAssist.mockImplementationOnce(async (_payload, onChunk?: OnChunk) => {
      onChunk?.('B chunk');
      return assistOk('B chunk', 'Q2');
    });
    await ask('Q2');

    // A's late chunk arrives after B already owns the buffer — must be dropped.
    chunkA?.('A late chunk');

    // A finally settles — its terminal write must not clobber B's buffer.
    resolveA?.(assistOk('A full answer', 'Q1'));
    await runA;

    expect(await bufferNow()).toEqual(progress('B chunk', true, false));
  });
});
