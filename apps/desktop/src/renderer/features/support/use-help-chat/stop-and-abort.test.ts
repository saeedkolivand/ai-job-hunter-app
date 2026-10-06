/**
 * useHelpChat — stopping, superseding and single-flight: how the hook treats an
 * answer stream that is cut short or overtaken.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { generateHelpAnswer } from '@/lib/generate';

import {
  HIT,
  hybridReply,
  pendingSearch,
  renderChat,
  resetChatMocks,
  startAndWaitStreaming,
} from './test-support';

vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('@ajh/prompts/generate', async (importOriginal) =>
  (await import('./test-mocks')).promptsMock(await importOriginal())
);

describe('useHelpChat — stop, supersede, single-flight', () => {
  beforeEach(resetChatMocks);

  it('stop() aborts the stream and keeps the partial answer as the assistant turn', async () => {
    let abortSignal: AbortSignal | undefined;
    let release: (() => void) | undefined;
    vi.mocked(generateHelpAnswer).mockImplementationOnce(
      ({ onToken, signal }) =>
        new Promise((resolve) => {
          abortSignal = signal;
          onToken?.('Open the ');
          release = () => resolve('never used');
        })
    );

    const { result } = renderChat();
    let pending: Promise<boolean> | undefined;
    await act(async () => {
      pending = result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.answer).toBe('Open the '));
    });

    act(() => {
      result.current.stop();
    });

    expect(abortSignal?.aborted).toBe(true);
    expect(result.current.streaming).toBe(false);
    expect(result.current.answer).toBe('');
    // The half-written answer is kept, WITH the sources retrieval had already
    // settled — losing it on Stop would throw away what the user asked for.
    expect(result.current.turns[1]?.content).toBe('Open the ');
    expect(result.current.turns[1]?.sources?.map((s) => s.id)).toEqual(['exportDoc']);

    await act(async () => {
      release?.();
      await pending;
    });
    // The resolved-after-abort value must NOT append a second assistant turn.
    expect(result.current.turns).toHaveLength(2);
  });

  it('a Stop during RETRIEVAL keeps the busy state until help_search settles', async () => {
    const pending = pendingSearch();
    const { result } = renderChat('llama3:70b', { 'help.search': pending.search });
    await startAndWaitStreaming(result, 'how do i export a pdf');

    act(() => {
      result.current.stop();
    });

    // The cancel below makes the backend give up SOONER, not instantly (it is
    // checked between dense candidates), and the invoke promise is not
    // abortable either way. Handing the Ask button back HERE is what lets a
    // second cold search run alongside the first one.
    expect(result.current.streaming).toBe(true);
    expect(pending.search).toHaveBeenCalledTimes(1);

    await act(async () => {
      pending.settle?.(hybridReply(HIT));
      await waitFor(() => expect(result.current.streaming).toBe(false));
    });

    // …and the abort still did its half of the job: the reply was dropped, so
    // no model call and no assistant turn came out of the stopped question.
    expect(generateHelpAnswer).not.toHaveBeenCalled();
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user']);
  });

  it('a second question aborts the run it replaces — one assistant turn, not two', async () => {
    const streams: AbortSignal[] = [];
    let releaseFirst: ((value: string) => void) | undefined;
    vi.mocked(generateHelpAnswer)
      .mockImplementationOnce(
        ({ signal }) =>
          new Promise((resolve) => {
            if (signal) streams.push(signal);
            releaseFirst = resolve;
          })
      )
      .mockImplementation(() => Promise.resolve('second answer'));

    const { result } = renderChat();
    // The handle as a component captured it BEFORE the first run re-rendered:
    // its `streaming` guard is a closed-over `false`, so both calls get in.
    // That is the only way two runs overlap, and it is why `run` must abort the
    // controller it replaces instead of just overwriting the ref.
    const staleSend = result.current.send;

    await startAndWaitStreaming(result, 'first question', staleSend);

    await act(async () => {
      await staleSend('second question');
    });

    expect(streams[0]?.aborted).toBe(true);
    const assistant = () => result.current.turns.filter((turn) => turn.role === 'assistant');
    expect(assistant().map((turn) => turn.content)).toEqual(['second answer']);

    // The abandoned stream resolving late must not append a turn of its own.
    await act(async () => {
      releaseFirst?.('first answer');
      await Promise.resolve();
    });
    expect(assistant()).toHaveLength(1);
  });

  it('refuses a new question while one is in flight (single-flight)', async () => {
    vi.mocked(generateHelpAnswer).mockImplementationOnce(() => new Promise(() => {}));
    const { result, mock } = renderChat();
    await startAndWaitStreaming(result, 'first question');

    // A FRESH handle, from a render where `streaming` is true: the guard inside
    // `run` is the only thing between the user and a second overlapping
    // question here — the Ask button is not even on screen.
    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.send('second question');
    });

    expect(answered).toBe(false);
    expect(mock.help.search).toHaveBeenCalledTimes(1);
    expect(result.current.turns.map((turn) => turn.content)).toEqual(['first question']);
  });

  it('aborts an in-flight stream on unmount', async () => {
    let abortSignal: AbortSignal | undefined;
    vi.mocked(generateHelpAnswer).mockImplementationOnce(
      ({ signal }) =>
        new Promise(() => {
          abortSignal = signal;
        })
    );

    const { result, unmount } = renderChat();
    await startAndWaitStreaming(result, 'how do i export a pdf');

    unmount();
    expect(abortSignal?.aborted).toBe(true);
  });
});
