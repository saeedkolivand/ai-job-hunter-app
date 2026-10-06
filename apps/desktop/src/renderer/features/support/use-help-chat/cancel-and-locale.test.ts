/**
 * useHelpChat — cancellation of the retrieval leg, and the per-question locale.
 *
 * `help_search` runs a dense arm that embeds one entry per cache miss, and a
 * Tauri invoke is not abortable from the renderer. `jobs.cancel(queryId)` is
 * the ONLY way to stop that work; the id is the only thing that names it, which
 * is why the shape of the id is asserted and not just its presence.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import i18n from '@ajh/translations';

import { generateHelpAnswer } from '@/lib/generate';

import { live } from './test-mocks';
import {
  ask,
  generateArg,
  HIT,
  hybridReply,
  pendingSearch,
  renderChat,
  resetChatMocks,
  searchArg,
  searchArgAt,
  startAndWaitStreaming,
} from './test-support';

vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('@ajh/prompts/generate', async (importOriginal) =>
  (await import('./test-mocks')).promptsMock(await importOriginal())
);
vi.mock('@ajh/translations', async (importOriginal) =>
  (await import('./test-mocks')).translationsMock(await importOriginal())
);

/** Switch the UI language inside `act`. */
const changeLanguage = (lng: string) =>
  act(async () => {
    await i18n.changeLanguage(lng);
  });

describe('useHelpChat — locale per question', () => {
  beforeEach(resetChatMocks);

  it('mints a distinct help- queryId per question and sends the active UI locale', async () => {
    const { result, mock } = renderChat();
    const search = mock.help.search as ReturnType<typeof vi.fn>;

    await ask(result, 'how do i export a pdf');
    await ask(result, 'and how do i track a job');

    const first = searchArgAt(search, 0);
    const second = searchArgAt(search, 1);
    for (const req of [first, second]) {
      // The prefix is what keeps this id from naming a live `job-`/`run-`/
      // `search-` run in the shared cancel registry; the cap is the schema's.
      expect(req.queryId.startsWith('help-')).toBe(true);
      expect(req.queryId.length).toBeLessThanOrEqual(64);
      // A full BCP-47 tag, region and all (`en-US` here) — the Rust side
      // normalises to the primary subtag, so this is deliberately not pinned
      // to a bare `en`.
      expect(req.locale).toMatch(/^en(-|$)/);
    }
    // Per QUESTION, not per session: a reused id would let a cancel for the
    // first question kill the second one's retrieval.
    expect(second.queryId).not.toBe(first.queryId);
  });

  it('follows the UI language rather than sending a fixed locale', async () => {
    // Without this the assertion above passes for a hardcoded 'en': the whole
    // point of the field is that a German corpus is filtered with the German
    // function-word list.
    await changeLanguage('de');
    try {
      const { result, mock } = renderChat();
      await ask(result, 'wie exportiere ich ein pdf');
      expect(searchArg(mock).locale).toMatch(/^de(-|$)/);
    } finally {
      await changeLanguage('en-US');
    }
  });

  it('pins ONE locale per question, even if the UI language changes mid-retrieval', async () => {
    // The run reads the language TWICE — once for the request that picks the
    // function-word list, once for the answer's output language — either side
    // of an await that lasts as long as the dense arm does. Read live, a switch
    // inside that window filters the question in English and then answers it in
    // German: two halves of one answer in different locales, with nothing in
    // the UI saying so.
    //
    // Run against the LIVE i18n instance — see the `live` mock, without which
    // the switch below is invisible to the hook and this test passes on the
    // broken code.
    live.i18n = true;
    const asked = i18n.language;
    expect(asked).toMatch(/^en(-|$)/);

    const pending = pendingSearch();
    try {
      const { result } = renderChat('llama3:70b', { 'help.search': pending.search });
      await startAndWaitStreaming(result, 'how do i export a pdf');

      // The switch lands while retrieval is still pending — the whole point.
      await changeLanguage('de');
      expect(i18n.language).toBe('de');

      await act(async () => {
        pending.settle?.(hybridReply(HIT));
        await waitFor(() => expect(result.current.streaming).toBe(false));
      });

      // Both legs carry the locale the question was ASKED in, not the one the
      // UI drifted to while the dense arm was still embedding.
      expect(searchArgAt(pending.search, 0).locale).toBe(asked);
      expect(generateArg().language).toBe(asked);
    } finally {
      live.i18n = false;
      await changeLanguage(asked);
    }
  });
});

describe('useHelpChat — cancelling the retrieval leg', () => {
  beforeEach(resetChatMocks);

  it('a Stop during RETRIEVAL cancels the backend leg by the id it sent', async () => {
    const pending = pendingSearch();
    const { result, mock } = renderChat('llama3:70b', { 'help.search': pending.search });
    await startAndWaitStreaming(result, 'how do i export a pdf');
    const { queryId } = searchArgAt(pending.search, 0);

    act(() => {
      result.current.stop();
    });

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(queryId));

    // Once the leg settles the id names nothing, so a later Stop must not fire
    // a second cancel — that would be a no-op that still emits a jobs event.
    await act(async () => {
      pending.settle?.(hybridReply(HIT));
      await waitFor(() => expect(result.current.streaming).toBe(false));
    });
    act(() => {
      result.current.stop();
    });
    expect(mock.jobs.cancel).toHaveBeenCalledTimes(1);
  });

  it('unmounting mid-RETRIEVAL cancels the leg — navigating away is the common case', async () => {
    const search = vi.fn().mockImplementation(() => new Promise(() => {}));
    const { result, mock, unmount } = renderChat('llama3:70b', { 'help.search': search });
    await startAndWaitStreaming(result, 'how do i export a pdf');
    const { queryId } = searchArgAt(search, 0);

    unmount();

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(queryId));
  });

  it('unmounting mid-STREAM does not cancel — the retrieval leg is already done', async () => {
    vi.mocked(generateHelpAnswer).mockImplementationOnce(() => new Promise(() => {}));
    const { result, mock, unmount } = renderChat();

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(generateHelpAnswer).toHaveBeenCalled());
    });

    unmount();
    // A bare assertion straight after `unmount()` would pass for the wrong
    // reason: `mutateAsync` reaches the client several microtasks later, so
    // nothing has been called yet either way. Wait past the point where the
    // mid-RETRIEVAL test's cancel has already landed.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // `jobs.cancel` on a finished id is not free: it emits `job.cancelled` for
    // an id with no job record and invalidates the whole jobs list.
    expect(mock.jobs.cancel).not.toHaveBeenCalled();
  });

  it('a second question cancels the retrieval of the run it replaces', async () => {
    const search = vi
      .fn()
      .mockImplementationOnce(() => new Promise(() => {}))
      .mockResolvedValue(hybridReply(HIT));
    const { result, mock } = renderChat('llama3:70b', { 'help.search': search });
    // The handle a component captured BEFORE the first run re-rendered: its
    // `streaming` guard is a closed-over `false`, so both calls get in.
    const staleSend = result.current.send;

    await startAndWaitStreaming(result, 'first question', staleSend);
    const first = searchArgAt(search, 0);

    await act(async () => {
      await staleSend('second question');
    });

    expect(mock.jobs.cancel).toHaveBeenCalledWith(first.queryId);
    expect(searchArgAt(search, 1).queryId).not.toBe(first.queryId);
  });

  it('a superseded run settling late leaves the SECOND question cancellable', async () => {
    // The test above leaves the first leg pending forever, so it cannot see
    // this: the superseded run only reaches its post-`await` bookkeeping when
    // it RESOLVES, and that is where an unguarded `queryIdRef.current = null`
    // wipes the id the replacing run had already minted — silently making the
    // question the user is actually waiting on uncancellable.
    let settleFirst: ((value: unknown) => void) | undefined;
    const search = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            settleFirst = resolve;
          })
      )
      // The second leg stays in retrieval so there is something for `stop()`
      // to cancel at the moment the first one settles.
      .mockImplementation(() => new Promise(() => {}));
    const { result, mock } = renderChat('llama3:70b', { 'help.search': search });
    const staleSend = result.current.send;

    await startAndWaitStreaming(result, 'first question', staleSend);

    await act(async () => {
      void staleSend('second question');
      await waitFor(() => expect(search).toHaveBeenCalledTimes(2));
    });
    const second = searchArgAt(search, 1);

    // The replaced run finishes now — after the supersede, not before.
    await act(async () => {
      settleFirst?.(hybridReply(HIT));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    act(() => {
      result.current.stop();
    });

    await waitFor(() => expect(mock.jobs.cancel).toHaveBeenCalledWith(second.queryId));
  });
});
