import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

// The generation half is the seam: stub it so the test asserts what the hook
// FEEDS the model (entries, glance, history) rather than re-testing streaming.
vi.mock('@/lib/generate', () => ({
  generateHelpAnswer: vi.fn().mockResolvedValue('Open the document and click Export.'),
}));

import { generateHelpAnswer } from '@/lib/generate';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useHelpChat } from './use-help-chat';

/** `exportDoc` lives in the aiGenerate section — NOT the applications one. */
const HIT = { id: 'exportDoc', score: 0.9 };
/** `trackJob` is a `support.faq.applicationsQuestions.*` entry. */
const APPLICATIONS_HIT = { id: 'trackJob', score: 0.9 };

function client(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  return createMockClient({
    'help.search': vi.fn().mockResolvedValue({
      results: [HIT],
      mode: 'hybrid',
      arms: { lexical: 'ran', dense: 'ran' },
    }),
    'ai.embeddingStatus': vi
      .fn()
      .mockResolvedValue({ documents: { total: 3, indexedInActiveSpace: 3, stale: 0 } }),
    'scrape.listInteractions': vi.fn().mockResolvedValue([
      { interactionType: 'viewed' },
      { interactionType: 'viewed' },
      // `dismissed` is NOT a tracked type — it must never reach the glance.
      { interactionType: 'dismissed' },
    ]),
    'applications.list': vi
      .fn()
      .mockResolvedValue([
        { id: 'a1', title: 'Senior Engineer', company: 'Acme', status: 'applied', updatedAt: 2 },
      ]),
    'autopilot.list': vi.fn().mockResolvedValue([{ id: 'ap1' }, { id: 'ap2' }]),
    ...overrides,
  });
}

/**
 * Render the hook. Nothing is awaited here on purpose: the four lists behind the
 * data glance are fetched inside `send`, not on mount, so there is no
 * "wait for the queries to land" step — see the privacy test below.
 *
 * `llama3` has no parseable parameter size, so `detectModelSize` classifies it
 * `small`; `llama3:70b` is the large-tier model in these tests.
 */
function render(model = 'llama3:70b', overrides = {}) {
  const mock = client(overrides);
  const rendered = renderHookWithClient(() => useHelpChat({ model, canUse: true }), {
    client: mock,
  });
  return { ...rendered, mock };
}

const searchArg = (mock: ReturnType<typeof client>) =>
  (mock.help.search as ReturnType<typeof vi.fn>).mock.calls[0]?.[0] as {
    query: string;
    entries: Array<{ id: string; title: string; body: string }>;
    limit: number;
  };

const generateArg = () =>
  vi.mocked(generateHelpAnswer).mock.calls[0]?.[0] as Parameters<typeof generateHelpAnswer>[0];

/** The four reads that make up the data glance. */
const dataReads = (mock: ReturnType<typeof client>) =>
  [
    mock.ai.embeddingStatus,
    mock.scrape.listInteractions,
    mock.applications.list,
    mock.autopilot.list,
  ] as ReturnType<typeof vi.fn>[];

describe('useHelpChat', () => {
  beforeEach(() => {
    vi.mocked(generateHelpAnswer).mockClear();
    vi.mocked(generateHelpAnswer).mockResolvedValue('Open the document and click Export.');
  });

  it('sends the whole active-locale corpus to help:search, then answers from the hits', async () => {
    const { result, mock } = render();

    await act(async () => {
      await result.current.send('  how do i export a pdf  ');
    });

    const req = searchArg(mock);
    // Trimmed, and the corpus is every shipped entry — Rust does the ranking,
    // the renderer only supplies the text.
    expect(req.query).toBe('how do i export a pdf');
    expect(req.entries.length).toBeGreaterThan(50);
    expect(req.entries.map((entry) => entry.id)).toContain('exportDoc');
    // The `limit` is the prompt builder's own entry budget for this profile.
    expect(req.limit).toBe(3);
    // ONLY the three fields the contract names travel: the section each entry
    // came from is a local routing hint, not part of the wire shape.
    expect(Object.keys(req.entries[0] ?? {}).sort()).toEqual(['body', 'id', 'title']);

    // Only the RANKED entry reaches the model, keyed back by id.
    const gen = generateArg();
    expect(gen.entries).toHaveLength(1);
    expect(gen.question).toBe('how do i export a pdf');
    expect(gen.model).toBe('llama3:70b');

    // Both turns land, and the answer carries its provenance.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user', 'assistant']);
    expect(result.current.turns[1]?.content).toBe('Open the document and click Export.');
    expect(result.current.turns[1]?.sources?.map((s) => s.id)).toEqual(['exportDoc']);
    expect(result.current.turns[1]?.mode).toBe('hybrid');
    expect(result.current.streaming).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('asks for fewer entries on a SMALL model (the prompt budget drives the request)', async () => {
    const { result, mock } = render('llama3.2:1b');
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    expect(searchArg(mock).limit).toBe(2);
  });

  it('builds the data glance from the user’s own counts, excluding untracked interactions', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('what have i done so far');
    });

    const glance = generateArg().dataGlance ?? '';
    expect(glance).toContain('Documents imported: 3');
    // Two `viewed`; the `dismissed` row is excluded by the tracked allowlist —
    // a dismissal is the opposite of tracking, so it must not inflate this.
    expect(glance).toContain('viewed 2');
    expect(glance).not.toContain('dismissed');
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).toContain('Autopilots configured: 2');
  });

  it('reads none of the user’s lists until a question is actually asked', async () => {
    const { result, mock } = render();

    // Opening the Help page to read ONE entry must not read the user's
    // documents, interactions, applications or autopilots. Mounting the four
    // queries would have issued all four before a question existed.
    for (const read of dataReads(mock)) expect(read).not.toHaveBeenCalled();

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    for (const read of dataReads(mock)) expect(read).toHaveBeenCalledTimes(1);
  });

  it('withholds the recent-application NAMES unless the question retrieved an applications entry', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    const glance = generateArg().dataGlance ?? '';
    // Counts are always safe to send; the scraped job titles and company names
    // are not, and an export question is not made better by them.
    expect(glance).toContain('Applications tracked: 1');
    expect(glance).not.toContain('Senior Engineer');
    expect(glance).not.toContain('Acme');
  });

  it('includes the recent-application names when an applications entry was retrieved', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [APPLICATIONS_HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      }),
    });
    await act(async () => {
      await result.current.send('which jobs have i applied to');
    });

    expect(generateArg().dataGlance ?? '').toContain('Senior Engineer — Acme (applied)');
  });

  it('passes the PRIOR turns as history, never the question being asked', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.send('first question');
    });
    await act(async () => {
      await result.current.send('second question');
    });

    const second = vi.mocked(generateHelpAnswer).mock.calls[1]?.[0];
    expect(second?.history?.map((turn) => turn.content)).toEqual([
      'first question',
      'Open the document and click Export.',
    ]);
    expect(second?.history?.map((turn) => turn.content)).not.toContain('second question');
  });

  it('flags a keyword-only answer on the turn, WITH the reason the dense arm did not run', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [HIT],
        mode: 'keyword',
        arms: { lexical: 'ran', dense: 'skipped' },
      }),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    expect(result.current.turns[1]?.mode).toBe('keyword');
    // `skipped` (the user's opt-out) and `unavailable` (an embedding failure)
    // both produce `mode: 'keyword'` but need different copy, so the arm status
    // has to survive onto the turn.
    expect(result.current.turns[1]?.dense).toBe('skipped');
  });

  it('carries dense=unavailable through, distinct from the opt-out', async () => {
    const { result } = render('llama3:70b', {
      'help.search': vi.fn().mockResolvedValue({
        results: [HIT],
        mode: 'keyword',
        arms: { lexical: 'ran', dense: 'unavailable' },
      }),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    expect(result.current.turns[1]?.dense).toBe('unavailable');
  });

  it('surfaces a retrieval failure and never asks the model to answer from nothing', async () => {
    const { result, mock } = render('llama3:70b', {
      'help.search': vi.fn().mockRejectedValue(new Error('help_search failed')),
    });

    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.send('how do i export a pdf');
    });

    // The boolean is what tells the UI to KEEP the typed question: a failed
    // question that was cleared from the box has to be retyped from memory.
    expect(answered).toBe(false);
    expect(result.current.error).toBe('help_search failed');
    expect(generateHelpAnswer).not.toHaveBeenCalled();
    // The question stays on screen; no assistant turn is fabricated.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user']);
    expect(result.current.streaming).toBe(false);
    // A failed retrieval never reaches the user's own lists either.
    for (const read of dataReads(mock)) expect(read).not.toHaveBeenCalled();
  });

  it('retry re-answers the failed question in place, without asking it twice', async () => {
    const search = vi
      .fn()
      .mockRejectedValueOnce(new Error('help_search failed'))
      .mockResolvedValue({
        results: [HIT],
        mode: 'hybrid',
        arms: { lexical: 'ran', dense: 'ran' },
      });
    const { result } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    expect(result.current.error).toBe('help_search failed');

    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.retry();
    });

    expect(answered).toBe(true);
    expect(result.current.error).toBeNull();
    // ONE user turn, not two: the failed question is already in the transcript.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user', 'assistant']);
    expect(result.current.turns[0]?.content).toBe('how do i export a pdf');
    expect(search.mock.calls[1]?.[0]).toMatchObject({ query: 'how do i export a pdf' });
    // …and the retried question is not fed back to the model as its own history.
    expect(generateArg().history).toEqual([]);
  });

  it('retry does nothing when there is no question to re-ask', async () => {
    const { result, mock } = render();
    let answered: boolean | undefined;
    await act(async () => {
      answered = await result.current.retry();
    });
    expect(answered).toBe(false);
    expect(mock.help.search).not.toHaveBeenCalled();
  });

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

    const { result } = render();
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
    let settle: ((value: unknown) => void) | undefined;
    const search = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        })
    );
    const { result } = render('llama3:70b', { 'help.search': search });

    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    act(() => {
      result.current.stop();
    });

    // `help_search` is a Tauri command — the abort cannot cancel it, only make
    // the reply be ignored. Handing the Ask button back HERE is what lets a
    // second cold search run alongside the first one.
    expect(result.current.streaming).toBe(true);
    expect(search).toHaveBeenCalledTimes(1);

    await act(async () => {
      settle?.({ results: [HIT], mode: 'hybrid', arms: { lexical: 'ran', dense: 'ran' } });
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

    const { result } = render();
    // The handle as a component captured it BEFORE the first run re-rendered:
    // its `streaming` guard is a closed-over `false`, so both calls get in.
    // That is the only way two runs overlap, and it is why `run` must abort the
    // controller it replaces instead of just overwriting the ref.
    const staleSend = result.current.send;

    await act(async () => {
      void staleSend('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

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
    const { result, mock } = render();

    await act(async () => {
      void result.current.send('first question');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

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

    const { result, unmount } = render();
    await act(async () => {
      void result.current.send('how do i export a pdf');
      await waitFor(() => expect(result.current.streaming).toBe(true));
    });

    unmount();
    expect(abortSignal?.aborted).toBe(true);
  });

  it('does nothing for a blank question or while AI is unavailable', async () => {
    const mock = client();
    const { result } = renderHookWithClient(
      () => useHelpChat({ model: 'llama3:70b', canUse: false }),
      { client: mock }
    );

    await act(async () => {
      await result.current.send('a real question');
    });
    expect(mock.help.search).not.toHaveBeenCalled();

    const usable = render();
    await act(async () => {
      await usable.result.current.send('   ');
    });
    expect(usable.mock.help.search).not.toHaveBeenCalled();
    expect(usable.result.current.turns).toHaveLength(0);
  });
});
