import { describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-autopilot';
import { useAutopilots, useBestMatches, useRemoveAutopilot } from './use-autopilot';

// gcTime: Infinity so cache seeded without an active observer is not collected.
const persistentClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: Infinity },
      mutations: { retry: false },
    },
  });

describe('use-autopilot services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── The fallback's freshness hole ────────────────────────────────────────────
//
// `AutopilotPage`'s `runState` fallback reads `ap.runStatus` off THIS query
// whenever its own run-state machine starts fresh (a remount). Nothing
// invalidates this query when a run STARTS, so the fallback is only honest if
// a remount always sees a genuinely fresh answer — which requires this hook's
// OWN `staleTime`, not the app-wide default, since the app-wide default (30s,
// see `query-client.ts`) is exactly what let a remount inside that window
// reuse a pre-run snapshot.
describe('useAutopilots — staleTime', () => {
  it('refetches on every mount even under a non-zero inherited staleTime default', async () => {
    const list = vi
      .fn()
      .mockResolvedValueOnce([{ _id: 'a', runStatus: 'completed' }])
      .mockResolvedValueOnce([{ _id: 'a', runStatus: 'inProgress' }]);
    const client = createMockClient({ 'autopilot.list': list });
    // Mirrors the app's real default (`query-client.ts`'s `staleTime:
    // QUERY_TIMES.MEDIUM`, 30s) — the test harness's OWN default QueryClient
    // (`staleTime: 0`) would mask the bug by construction, since everything
    // would look fresh-on-mount regardless of what this hook asks for.
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
    });

    const first = renderHookWithClient(() => useAutopilots(), { client, queryClient });
    await waitFor(() => expect(first.result.current.data?.[0]?.runStatus).toBe('completed'));
    first.unmount();

    // A second mount immediately after, with the SAME cache — the remount
    // scenario (navigate away and back), well inside the 30s window.
    const second = renderHookWithClient(() => useAutopilots(), { client, queryClient });
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(second.result.current.data?.[0]?.runStatus).toBe('inProgress'));
  });
});

describe('useBestMatches', () => {
  it('reads through api.autopilot.bestMatches() under the shared keys.autopilot.bestMatches key', async () => {
    const bestMatches = vi
      .fn()
      .mockResolvedValue({ matches: [{ key: 'k1' }], total: 1, autopilotCount: 1 });
    const client = createMockClient({ 'autopilot.bestMatches': bestMatches });
    const queryClient = persistentClient();

    const { result } = renderHookWithClient(() => useBestMatches(), { client, queryClient });

    await waitFor(() => expect(result.current.data?.total).toBe(1));
    expect(bestMatches).toHaveBeenCalledTimes(1);
    // Invalidating the PREFIX (what every autopilot mutation already does)
    // must refetch this query too — the whole point of nesting the key under
    // `autopilot.all` rather than a sibling top-level key.
    expect(queryClient.getQueryState(keys.autopilot.bestMatches)?.isInvalidated).toBe(false);
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: keys.autopilot.all });
    });
    expect(bestMatches).toHaveBeenCalledTimes(2);
  });
});

describe('useRemoveAutopilot — optimistic delete', () => {
  it('removes the card before the backend resolves, then rolls back on error', async () => {
    let reject!: (e: unknown) => void;
    const remove = vi.fn(() => new Promise((_res, rej) => (reject = rej)));
    const list = vi.fn().mockResolvedValue([{ _id: 'a' }, { _id: 'b' }]);
    const client = createMockClient({ 'autopilot.remove': remove, 'autopilot.list': list });
    const queryClient = persistentClient();
    queryClient.setQueryData(keys.autopilot.all, [{ _id: 'a' }, { _id: 'b' }]);

    const { result } = renderHookWithClient(() => useRemoveAutopilot(), { client, queryClient });

    act(() => result.current.mutate('a'));

    await waitFor(() =>
      expect(queryClient.getQueryData(keys.autopilot.all)).toEqual([{ _id: 'b' }])
    );

    act(() => reject(new Error('boom')));
    await waitFor(() =>
      expect(queryClient.getQueryData(keys.autopilot.all)).toEqual([{ _id: 'a' }, { _id: 'b' }])
    );
  });
});
