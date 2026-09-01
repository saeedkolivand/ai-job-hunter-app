/**
 * usePostingsSearch — the real UX wired onto the minimal `useHybridSearch`
 * mutation: queryId minting, cancel-before-supersede, out-of-order/`cancelled`
 * discarding, and the idle → searching → results/noResults/stale/error state
 * machine.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const notifyMock = { error: vi.fn(), success: vi.fn(), info: vi.fn(), warning: vi.fn() };
vi.mock('@ajh/ui', () => ({
  useNotification: () => notifyMock,
}));

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { usePostingsSearch } from './usePostingsSearch';

function okResult(hits: string[], overrides: Record<string, unknown> = {}) {
  return {
    outcome: 'ok',
    hits,
    arms: { lexical: 'ran', dense: 'ran', rerank: 'ran' },
    corpusSize: 10,
    ...overrides,
  };
}

function setup(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  return renderHookWithClient(() => usePostingsSearch(), { client: createMockClient(overrides) });
}

beforeEach(() => {
  usePreferencesStore.setState({ semanticScoring: false });
  notifyMock.error.mockClear();
});

describe('usePostingsSearch', () => {
  it('starts idle and transitions to results on a hit', async () => {
    const hybridSearch = vi.fn().mockResolvedValue(okResult(['a', 'b']));
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    expect(result.current.state).toBe('idle');
    act(() => result.current.search('engineer', ['a', 'b', 'c']));
    expect(result.current.state).toBe('searching');

    await waitFor(() => expect(result.current.state).toBe('results'));
    expect(result.current.result?.hits).toEqual(['a', 'b']);
    expect(result.current.committedQuery).toBe('engineer');
    expect(hybridSearch).toHaveBeenCalledWith(
      expect.objectContaining({ query: 'engineer', eligibleIds: ['a', 'b', 'c'], limit: 20 })
    );
  });

  it('a zero-hit ok outcome is noResults, never the generic empty state', async () => {
    const hybridSearch = vi.fn().mockResolvedValue(okResult([]));
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    act(() => result.current.search('zzz-no-match', []));
    await waitFor(() => expect(result.current.state).toBe('noResults'));
  });

  it('a staleCorpus outcome is its own state, not the generic error state', async () => {
    const hybridSearch = vi.fn().mockResolvedValue({
      outcome: 'staleCorpus',
      hits: [],
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'skipped' },
      corpusSize: 0,
    });
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    act(() => result.current.search('x', []));
    await waitFor(() => expect(result.current.state).toBe('stale'));
  });

  it('a rejected mutation goes to error', async () => {
    const hybridSearch = vi.fn().mockRejectedValue(new Error('boom'));
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    act(() => result.current.search('x', []));
    await waitFor(() => expect(result.current.state).toBe('error'));
  });

  it('cancels the previous queryId before firing the next search, and the stale response cannot clobber the new one', async () => {
    let resolveFirst: (v: unknown) => void = () => {};
    const hybridSearch = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirst = resolve;
          })
      )
      .mockResolvedValueOnce(okResult(['second']));
    const cancel = vi.fn().mockResolvedValue({ success: true });
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch, 'jobs.cancel': cancel });

    act(() => result.current.search('first', []));
    await waitFor(() => expect(hybridSearch).toHaveBeenCalledTimes(1));
    const firstQueryId = (hybridSearch.mock.calls[0]?.[0] as { queryId: string }).queryId;

    act(() => result.current.search('second', []));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith(firstQueryId));

    // The superseded first request resolving LATE must not overwrite the
    // second search's (already-settled) state — out-of-order IPC resolution.
    await waitFor(() => expect(result.current.state).toBe('results'));
    resolveFirst(okResult(['stale-should-be-ignored']));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current.result?.hits).toEqual(['second']);
  });

  it('discards an outcome: cancelled response instead of surfacing it as an error', async () => {
    const hybridSearch = vi.fn().mockResolvedValue({
      outcome: 'cancelled',
      hits: [],
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'skipped' },
      corpusSize: 0,
    });
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    act(() => result.current.search('x', []));
    await waitFor(() => expect(hybridSearch).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 0));
    // No SETTLED_* / FAILED event ever fires for 'cancelled' — the machine
    // stays exactly where SUBMIT left it.
    expect(result.current.state).toBe('searching');
    expect(result.current.result).toBeNull();
  });

  it('enableSemanticRanking flips the preference, mirrors it, and re-issues the last query', async () => {
    const hybridSearch = vi
      .fn()
      .mockResolvedValueOnce(
        okResult(['a'], { arms: { lexical: 'ran', dense: 'skipped', rerank: 'ran' } })
      )
      .mockResolvedValueOnce(okResult(['a', 'b']));
    const setSemanticScoring = vi.fn().mockResolvedValue(undefined);
    const { result } = setup({
      'scrape.hybridSearch': hybridSearch,
      'jobPreferences.setSemanticScoring': setSemanticScoring,
    });

    act(() => result.current.search('engineer', ['a', 'b']));
    await waitFor(() => expect(result.current.state).toBe('results'));

    act(() => result.current.enableSemanticRanking(['a', 'b']));

    expect(usePreferencesStore.getState().semanticScoring).toBe(true);
    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledWith(true));
    await waitFor(() => expect(hybridSearch).toHaveBeenCalledTimes(2));
    expect(hybridSearch.mock.calls[1]?.[0]).toMatchObject({ query: 'engineer' });
  });

  it('surfaces a failed semantic-scoring mirror write instead of swallowing it', async () => {
    // Regression for the HIGH finding: a failed backend sync must notify —
    // otherwise the local preference flips to on, the search re-runs and
    // shows results, and the user has zero indication the persisted mirror
    // never updated. This test FAILS if the `onError` handler is removed
    // from `enableSemanticRanking`'s `syncSemanticScoring.mutate(true, ...)`.
    const hybridSearch = vi.fn().mockResolvedValue(okResult(['a']));
    const setSemanticScoring = vi.fn().mockRejectedValue(new Error('offline'));
    const { result } = setup({
      'scrape.hybridSearch': hybridSearch,
      'jobPreferences.setSemanticScoring': setSemanticScoring,
    });

    act(() => result.current.search('engineer', ['a']));
    await waitFor(() => expect(result.current.state).toBe('results'));

    act(() => result.current.enableSemanticRanking(['a']));

    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledWith(true));
    await waitFor(() =>
      expect(notifyMock.error).toHaveBeenCalledWith({
        message: 'settings.embeddings.semanticScoringSyncFailed',
      })
    );
  });

  it('mints every queryId with the `search-` prefix Rust validates', async () => {
    const hybridSearch = vi.fn().mockResolvedValue(okResult(['a']));
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch });

    act(() => result.current.search('engineer', []));
    await waitFor(() => expect(hybridSearch).toHaveBeenCalledTimes(1));

    const queryId = (hybridSearch.mock.calls[0]?.[0] as { queryId: string }).queryId;
    expect(queryId.startsWith('search-')).toBe(true);
    expect(queryId.length).toBeLessThanOrEqual(64);
  });

  it('clear() resets to idle and cancels any in-flight search without touching the last query text', async () => {
    let resolveSearch: (v: unknown) => void = () => {};
    const hybridSearch = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveSearch = resolve;
        })
    );
    const cancel = vi.fn().mockResolvedValue({ success: true });
    const { result } = setup({ 'scrape.hybridSearch': hybridSearch, 'jobs.cancel': cancel });

    act(() => result.current.search('engineer', []));
    expect(result.current.state).toBe('searching');

    act(() => result.current.clear());
    expect(result.current.state).toBe('idle');
    expect(result.current.committedQuery).toBe('');
    await waitFor(() => expect(cancel).toHaveBeenCalled());

    // The abandoned search resolving after clear() must not resurrect it.
    resolveSearch(okResult(['late']));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current.state).toBe('idle');
  });
});
