import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
} from '@/test-support';

import * as mod from './use-discovery';
import { useCompanySearch, useSetStarred, useWatchedCompanies } from './use-discovery';

describe('use-discovery services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useCompanySearch', () => {
  it('passes the request through and returns the discovered rows', async () => {
    const searchCompanies = vi
      .fn()
      .mockResolvedValue([
        { atsKind: 'greenhouse', slug: 'stripe', seenCount: 3, starred: false, source: 'scrape' },
      ]);
    const client = createMockClient({ 'discovery.searchCompanies': searchCompanies });

    const { result } = renderHookWithClient(() => useCompanySearch({ query: 'str' }), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(searchCompanies).toHaveBeenCalledWith({ query: 'str' });
    expect(result.current.data?.[0]?.slug).toBe('stripe');
  });
});

describe('useWatchedCompanies', () => {
  it('returns the starred set', async () => {
    const watched = vi
      .fn()
      .mockResolvedValue([
        { atsKind: 'ashby', slug: 'Linear', seenCount: 0, starred: true, source: 'seed' },
      ]);
    const client = createMockClient({ 'discovery.watched': watched });

    const { result } = renderHookWithClient(() => useWatchedCompanies(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]?.slug).toBe('Linear');
  });
});

describe('useSetStarred', () => {
  /**
   * The real Tauri failure mode: the command RESOLVES `json!({"error": ...})` as
   * a value (never rejects), so a naive mutationFn would fire `onSuccess`. The
   * hook must narrow the `{ error }` union and throw — so no invalidation runs
   * (the #756 lesson).
   */
  it('throws and does NOT invalidate when the command RESOLVES an { error } payload', async () => {
    const setStarred = vi.fn().mockResolvedValue({ error: 'boom' });
    const client = createMockClient({ 'discovery.setStarred': setStarred });
    const queryClient = makeQueryClient();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useSetStarred(), { client, queryClient });

    await act(async () => {
      await expect(
        result.current.mutateAsync({ atsKind: 'greenhouse', slug: 'stripe', starred: true })
      ).rejects.toThrow('boom');
    });

    expect(invalidateSpy).not.toHaveBeenCalled();
  });

  it('invalidates the discovery queries on a real success', async () => {
    const setStarred = vi.fn().mockResolvedValue({ success: true });
    const client = createMockClient({ 'discovery.setStarred': setStarred });
    const queryClient = makeQueryClient();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useSetStarred(), { client, queryClient });

    await act(async () => {
      await result.current.mutateAsync({ atsKind: 'greenhouse', slug: 'stripe', starred: true });
    });

    const invalidatedKeys = invalidateSpy.mock.calls.map(
      (call) => (call[0] as { queryKey?: unknown } | undefined)?.queryKey
    );
    expect(invalidatedKeys).toContainEqual(['discovery']);
  });
});
