// "Help me answer…" answer.assist ↔ answer.assist.result — the first
// BILLABLE-AI verb on the bridge, and the only one that streams.

import { describe, expect, it, vi } from 'vitest';

import {
  connectedClient,
  type FakeWebSocket,
  frameAt,
  itMalformed,
  itRoundTrips,
  outcomeOf,
  reply,
  sendAndAwaitFrame,
  setupFakeWebSocket,
  T,
  type VerbCase,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

const REQUEST = {
  question: 'Why do you want this role?',
  url: 'https://jobs.example.com/posting/1',
  searchWeb: false,
};
const verb: VerbCase = {
  call: (c) => c.answerAssist(REQUEST),
  sentType: T.answerAssist,
  sentPayload: REQUEST,
  replyType: T.answerAssistResult,
};

const answered = (question: string, draft: string) => ({ ok: true, question, draft, sourced: {} });
const chunk = (socket: FakeWebSocket, reqId: string, delta: string): void =>
  reply(socket, T.assistChunk, reqId, { delta });
const done = (socket: FakeWebSocket, reqId: string): void =>
  reply(socket, T.assistDone, reqId, null);

/** Connect and start an answerAssist; the deltas the caller's `onChunk` sees land in `deltas`. */
async function startAssist() {
  const { client, socket } = await connectedClient(fake);
  const deltas: string[] = [];
  const { promise, frame } = await sendAndAwaitFrame(socket, () =>
    client.answerAssist({ question: 'Why this role?' }, (d) => deltas.push(d))
  );
  return { client, socket, deltas, promise, reqId: frame.reqId };
}

describe('BridgeClient – answerAssist', () => {
  itRoundTrips(fake, verb, [
    [
      'round-trips a success result into the resolved payload',
      {
        ok: true,
        question: REQUEST.question,
        draft: 'I am drawn to this role because…',
        sourced: { web: false, brief: true, salary: false },
      },
    ],
    [
      'round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects',
      { ok: false, error: 'AI answer drafting is off.' },
    ],
  ]);
  // draft must be a string; a number breaks the guard.
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when the payload is bad',
        { ok: true, question: REQUEST.question, draft: 42, sourced: {} },
      ],
    ],
    'ok'
  );

  it('forwards assist.chunk deltas to onChunk, correlated by reqId, before the terminal result', async () => {
    const { client, socket, deltas, promise, reqId } = await startAssist();

    chunk(socket, reqId, 'Because I ');
    chunk(socket, reqId, 'am drawn to it.');
    expect(deltas).toEqual(['Because I ', 'am drawn to it.']);

    done(socket, reqId);
    const final = answered('Why this role?', 'Because I am drawn to it.');
    reply(socket, T.answerAssistResult, reqId, final);
    expect(await promise).toEqual(final);
    client.dispose();
  });

  it('never forwards a chunk once the request has already settled', async () => {
    const { client, socket, deltas, promise, reqId } = await startAssist();

    reply(socket, T.answerAssistResult, reqId, answered('Why this role?', 'Because I am drawn.'));
    await promise;

    // A stray late chunk after settle must never fire the (already-forgotten) callback.
    chunk(socket, reqId, 'too late');
    expect(deltas).toEqual([]);
    client.dispose();
  });

  it('cancelAssist sends assist.cancel for the given reqId and stops further chunk delivery', async () => {
    const { client, socket, deltas, promise, reqId } = await startAssist();

    client.cancelAssist(reqId);
    const cancel = frameAt(socket);
    expect(cancel.type).toBe(T.assistCancel);
    expect(cancel.reqId).toBe(reqId);
    expect(cancel.payload).toBeNull();

    // A chunk arriving after the cancel must no longer reach the callback.
    chunk(socket, reqId, 'ignored');
    expect(deltas).toEqual([]);

    // Settle the still-pending promise so it doesn't dangle across tests.
    reply(socket, T.answerAssistResult, reqId, { ok: false, error: 'cancelled' });
    await promise;
    client.dispose();
  });

  it('cancelCurrent (PR4) retires the pending answerAssist exactly like a superseding call does', async () => {
    const { client, socket, promise, reqId } = await startAssist();

    client.cancelCurrent();

    expect(await promise).toEqual({ ok: false, error: 'Superseded by a newer request.' });
    // Sends the same assist.cancel a direct cancelAssist(reqId) call would.
    const cancel = frameAt(socket);
    expect(cancel.type).toBe(T.assistCancel);
    expect(cancel.reqId).toBe(reqId);
    client.dispose();
  });

  it('cancelCurrent (PR4) is a no-op when nothing is pending', async () => {
    const { client } = await connectedClient(fake);
    expect(() => client.cancelCurrent()).not.toThrow();
    client.dispose();
  });

  // ── supersede: a newer call must retire a still-pending older one, not leave
  // its promise dangling or its stale chunks able to fire ───────────────────────

  it('a newer answerAssist call settles a still-pending older one and ignores its stale chunks', async () => {
    const { client, socket, deltas: deltasA, promise: resultA, reqId: reqA } = await startAssist();

    const deltasB: string[] = [];
    const resultB = client.answerAssist({ question: 'Second question?' }, (d) => deltasB.push(d));

    // The OLDER request must settle immediately — never dangle until the stall
    // timeout — with a fixed `{ok:false}` result.
    expect(await resultA).toEqual({ ok: false, error: 'Superseded by a newer request.' });

    // Superseding must also best-effort cancel the OLD reqId server-side, so the
    // desktop actually stops streaming/charging it too.
    const sent = Array.from({ length: socket.send.mock.calls.length }, (_, i) =>
      frameAt(socket, i)
    );
    expect(sent.some((f) => f.type === T.assistCancel && f.reqId === reqA)).toBe(true);

    // A late chunk for the retired OLD reqId must never reach its callback — it
    // can no longer mutate whatever shared buffer the caller accumulates into.
    chunk(socket, reqA, 'stale, must be ignored');
    expect(deltasA).toEqual([]);

    // The NEW request must still work completely normally.
    await vi.waitFor(() => expect(socket.send).toHaveBeenCalledTimes(5)); // hello + auth + A + cancel A + B
    const frameB = frameAt(socket);
    expect(frameB.type).toBe(T.answerAssist);

    chunk(socket, frameB.reqId, 'fresh');
    expect(deltasB).toEqual(['fresh']);

    const final = answered('Second question?', 'final answer');
    reply(socket, T.answerAssistResult, frameB.reqId, final);
    expect(await resultB).toEqual(final);
    client.dispose();
  });

  // ── stall timeout: reset on activity, cancels on a true stall ──────────────

  it('a chunk resets the stall timer — activity past the old flat 30s window keeps the promise alive', async () => {
    vi.useFakeTimers();
    const { client, socket, promise, reqId } = await startAssist();

    // Two 40s hops (80s total, well past the OLD flat 30s timeout), each under the
    // NEW 60s stall window, with a chunk resetting the clock between them.
    await vi.advanceTimersByTimeAsync(40_000);
    chunk(socket, reqId, 'still going');
    await vi.advanceTimersByTimeAsync(40_000);

    done(socket, reqId);
    const final = answered('Why this role?', 'final answer');
    reply(socket, T.answerAssistResult, reqId, final);
    expect(await promise).toEqual(final);
    client.dispose();
  });

  it('a true stall — no chunk, no reply — for the full window sends assist.cancel and rejects', async () => {
    vi.useFakeTimers();
    const { client, socket, promise, reqId } = await startAssist();
    const outcomePromise = outcomeOf(promise);

    await vi.advanceTimersByTimeAsync(60_000);

    const outcome = await outcomePromise;
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) {
      expect(outcome.error).toBeInstanceOf(Error);
      expect((outcome.error as Error).message).toMatch(/timed out/i);
    }

    // The stall must cancel the desktop's stream too (stop streaming/charging),
    // not just give up client-side.
    const cancel = frameAt(socket);
    expect(cancel.type).toBe(T.assistCancel);
    expect(cancel.reqId).toBe(reqId);
    client.dispose();
  });
});
