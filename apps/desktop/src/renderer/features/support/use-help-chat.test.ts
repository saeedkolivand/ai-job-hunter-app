import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

// The generation half is the seam: stub it so the test asserts what the hook
// FEEDS the model (entries, glance, history) rather than re-testing streaming.
vi.mock('@/lib/generate', () => ({
  generateHelpAnswer: vi.fn().mockResolvedValue('Open the document and click Export.'),
}));

import { generateHelpAnswer } from '@/lib/generate';
import { keys } from '@/services/query-client';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useHelpChat } from './use-help-chat';

const HIT = { id: 'exportDoc', score: 0.9 };

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
 * Render the hook and wait for the four queries backing the data glance to have
 * LANDED (not merely been called) — a glance built from a still-pending query is
 * all zeros, which would let a broken glance pass as an empty one.
 *
 * `llama3` has no parseable parameter size, so `detectModelSize` classifies it
 * `small`; `llama3:70b` is the large-tier model in these tests.
 */
async function render(model = 'llama3:70b', overrides = {}) {
  const mock = client(overrides);
  const rendered = renderHookWithClient(() => useHelpChat({ model, canUse: true }), {
    client: mock,
  });
  const { queryClient } = rendered;
  await waitFor(() => {
    expect(queryClient.getQueryData(keys.ai.embeddingStatus)).toBeDefined();
    expect(queryClient.getQueryData(keys.postings.interactions(undefined))).toBeDefined();
    expect(queryClient.getQueryData(keys.applications.all)).toBeDefined();
    expect(queryClient.getQueryData(keys.autopilot.all)).toBeDefined();
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

describe('useHelpChat', () => {
  beforeEach(() => {
    vi.mocked(generateHelpAnswer).mockClear();
    vi.mocked(generateHelpAnswer).mockResolvedValue('Open the document and click Export.');
  });

  it('sends the whole active-locale corpus to help:search, then answers from the hits', async () => {
    const { result, mock } = await render();

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
    const { result, mock } = await render('llama3.2:1b');
    await act(async () => {
      await result.current.send('how do i export a pdf');
    });
    expect(searchArg(mock).limit).toBe(2);
  });

  it('builds the data glance from the user’s own counts, excluding untracked interactions', async () => {
    const { result } = await render();
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
    expect(glance).toContain('Senior Engineer — Acme (applied)');
    expect(glance).toContain('Autopilots configured: 2');
  });

  it('passes the PRIOR turns as history, never the question being asked', async () => {
    const { result } = await render();
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

  it('flags a keyword-only answer on the turn itself', async () => {
    const { result } = await render('llama3:70b', {
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
  });

  it('surfaces a retrieval failure and never asks the model to answer from nothing', async () => {
    const { result } = await render('llama3:70b', {
      'help.search': vi.fn().mockRejectedValue(new Error('help_search failed')),
    });

    await act(async () => {
      await result.current.send('how do i export a pdf');
    });

    expect(result.current.error).toBe('help_search failed');
    expect(generateHelpAnswer).not.toHaveBeenCalled();
    // The question stays on screen; no assistant turn is fabricated.
    expect(result.current.turns.map((turn) => turn.role)).toEqual(['user']);
    expect(result.current.streaming).toBe(false);
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

    const { result } = await render();
    let pending: Promise<void> | undefined;
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

  it('aborts an in-flight stream on unmount', async () => {
    let abortSignal: AbortSignal | undefined;
    vi.mocked(generateHelpAnswer).mockImplementationOnce(
      ({ signal }) =>
        new Promise(() => {
          abortSignal = signal;
        })
    );

    const { result, unmount } = await render();
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

    const usable = await render();
    await act(async () => {
      await usable.result.current.send('   ');
    });
    expect(usable.mock.help.search).not.toHaveBeenCalled();
    expect(usable.result.current.turns).toHaveLength(0);
  });
});
