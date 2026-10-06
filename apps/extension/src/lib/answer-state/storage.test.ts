/**
 * The impure surface of the answer state (`answer-state.ts`): the per-tab
 * update queue and the subscription's read/change ordering.
 */

import { describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import {
  type AnswerState,
  answerStateKey,
  subscribeAnswerState,
  updateAnswerState,
  writeAnswerState,
} from '../answer-state';

// `updateAnswerState`/`subscribeAnswerState` are the only impure surface this
// file exercises — an in-memory `storage.session` area (plus a capturable
// `onChanged` listener) is enough for that, and is inert for every other
// (pure) test below.
vi.mock('@wxt-dev/browser', () => {
  const store: Record<string, unknown> = {};
  return {
    browser: {
      storage: {
        session: {
          get: vi.fn((key: string) => Promise.resolve({ [key]: store[key] })),
          set: vi.fn((entries: Record<string, unknown>) => {
            Object.assign(store, entries);
            return Promise.resolve();
          }),
          remove: vi.fn((key: string) => {
            delete store[key];
            return Promise.resolve();
          }),
        },
        onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
      },
    },
  };
});

/** Flush a couple of microtask/timer turns for fire-and-forget async work
 *  (`void readAnswerState(...).then(...)`) to settle. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

const tabState = (tabId: number): AnswerState => ({
  tabId,
  origin: 'https://jobs.example.com',
  scannedAt: 0,
  rows: [],
  stream: null,
  pageChanged: false,
});

describe('updateAnswerState (per-tab queue)', () => {
  it('a throwing mutate rejects its own call without wedging the next call for the same tab', async () => {
    const tabId = 501;
    await writeAnswerState(tabState(tabId));

    await expect(
      updateAnswerState(tabId, () => {
        throw new Error('boom');
      })
    ).rejects.toThrow('boom');

    // The queue must have recovered from the rejection above — a call for the
    // SAME tab right after it should still complete, not hang forever behind
    // a wedged `prior`.
    const next = await updateAnswerState(tabId, (state) => ({ ...state, pageChanged: true }));
    expect(next?.pageChanged).toBe(true);
  });
});

describe('subscribeAnswerState', () => {
  it('a change delivered while the initial read is still pending is not clobbered by that read resolving late', async () => {
    const tabId = 502;
    const key = answerStateKey(tabId);
    const stale = tabState(tabId);
    await writeAnswerState(stale);

    const sessionGetMock = vi.mocked(browser.storage.session.get);
    const realGet = sessionGetMock.getMockImplementation();
    if (!realGet) throw new Error('expected the default storage.session.get mock');

    // Gate the INITIAL read behind a promise this test controls, so a change
    // event can be delivered while it is still in flight.
    let releaseRead: (() => void) | undefined;
    const readGate = new Promise<void>((resolve) => {
      releaseRead = resolve;
    });
    sessionGetMock.mockImplementationOnce(((k: string) =>
      readGate.then(() => realGet(k))) as typeof realGet);

    const seen: (AnswerState | null)[] = [];
    const unsubscribe = subscribeAnswerState(tabId, (state) => seen.push(state));

    // A fresher change lands WHILE the read above is still gated.
    const fresh = { ...stale, pageChanged: true };
    const addListenerMock = vi.mocked(browser.storage.onChanged.addListener);
    const listener = addListenerMock.mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('expected subscribeAnswerState to register a listener');
    listener({ [key]: { newValue: fresh, oldValue: stale } } as never, 'session');

    // Now let the stale read resolve.
    releaseRead?.();
    await flush();
    sessionGetMock.mockImplementation(realGet);
    unsubscribe();

    // The stale read's value must never have reached `onState` after the
    // fresher change already did — only the fresh state was ever delivered.
    expect(seen).toEqual([fresh]);
  });
});
