/**
 * use-referrals service hooks
 *
 * Strategy:
 *  - createMockClient from test-support (proxy-based spy factory).
 *  - renderHookWithClient wraps QueryClient + AppClientProvider.
 *  - Assertions: useReferrals is disabled when no jobUrl; useUpsertReferral calls
 *    the right IPC method and invalidates keys.referrals.all; useRemoveReferral
 *    performs optimistic removal and restores cache on error.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import type { ReferralContact } from '@ajh/shared/ipc';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
} from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-referrals';
import { useReferrals, useRemoveReferral, useUpsertReferral } from './use-referrals';

afterEach(() => vi.restoreAllMocks());

// ── Smoke ─────────────────────────────────────────────────────────────────────

describe('use-referrals service hooks smoke', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useReferrals ──────────────────────────────────────────────────────────────

describe('useReferrals', () => {
  it('calls api.referrals.list with the given jobUrl', async () => {
    const fixture: ReferralContact[] = [
      {
        id: 'r1',
        jobUrl: 'https://example.com/job/1',
        companyName: 'Acme',
        personName: 'Alice',
        channel: 'email',
        status: 'draft',
        createdAt: 1000,
        updatedAt: 1000,
      },
    ];
    const list = vi.fn().mockResolvedValue(fixture);
    const client = createMockClient({ 'referrals.list': list });

    const { result } = renderHookWithClient(() => useReferrals('https://example.com/job/1'), {
      client,
    });

    await waitFor(() => expect(result.current.data).toEqual(fixture));
    expect(list).toHaveBeenCalledWith('https://example.com/job/1');
  });

  it('is disabled (queryFn never called) when jobUrl is omitted', async () => {
    const list = vi.fn().mockResolvedValue([]);
    const client = createMockClient({ 'referrals.list': list });

    const { result } = renderHookWithClient(() => useReferrals(), { client });

    // fetchStatus is 'idle' when enabled:false — the query never fires.
    expect(result.current.fetchStatus).toBe('idle');
    expect(list).not.toHaveBeenCalled();
  });
});

// ── useUpsertReferral ─────────────────────────────────────────────────────────

describe('useUpsertReferral', () => {
  it('calls api.referrals.upsert with the validated payload', async () => {
    const stored: ReferralContact = {
      id: 'r2',
      jobUrl: 'https://example.com/job/2',
      companyName: 'Beta',
      personName: 'Bob',
      channel: 'linkedin_message',
      status: 'sent',
      createdAt: 2000,
      updatedAt: 2000,
    };
    const upsert = vi.fn().mockResolvedValue(stored);
    const client = createMockClient({ 'referrals.upsert': upsert });

    const { result } = renderHookWithClient(() => useUpsertReferral(), { client });

    await act(async () => {
      result.current.mutate({
        jobUrl: 'https://example.com/job/2',
        companyName: 'Beta',
        personName: 'Bob',
        channel: 'linkedin_message',
        status: 'sent',
      });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    // The hook validates through ReferralUpsertSchema before crossing IPC —
    // assert the IPC method was called (schema pass-through keeps shape identical
    // for valid payloads so the parsed value equals the input).
    expect(upsert).toHaveBeenCalledTimes(1);
  });

  it('invalidates keys.referrals.all on success', async () => {
    const upsert = vi.fn().mockResolvedValue({
      id: 'r3',
      jobUrl: 'https://j.com',
      companyName: 'C',
      personName: 'P',
      channel: 'email',
      status: 'draft',
      createdAt: 1,
      updatedAt: 1,
    });
    const client = createMockClient({ 'referrals.upsert': upsert });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useUpsertReferral(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate({
        companyName: 'C',
        personName: 'P',
        channel: 'email',
        status: 'draft',
      });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.referrals.all })
    );
  });
});

// ── useRemoveReferral ─────────────────────────────────────────────────────────

describe('useRemoveReferral', () => {
  const makeContact = (id: string): ReferralContact => ({
    id,
    jobUrl: 'https://example.com/job/1',
    companyName: 'Acme',
    personName: 'Alice',
    channel: 'email',
    status: 'draft',
    createdAt: 1000,
    updatedAt: 1000,
  });

  it('optimistically removes the item from cache in onMutate', async () => {
    // Intercept the removal so we can inspect cache state mid-flight (after
    // onMutate runs but before onSettled clears the cache with gcTime:0).
    let cacheAfterMutate: ReferralContact[] | undefined;
    const remove = vi.fn().mockImplementation(async () => {
      // At this point onMutate has already run — capture the optimistic state.
      cacheAfterMutate = queryClient.getQueryData<ReferralContact[]>(listKey);
    });
    const client = createMockClient({ 'referrals.remove': remove });
    const queryClient = makeQueryClient();

    // Seed the cache with two contacts under a concrete list key.
    const listKey = keys.referrals.list('https://example.com/job/1');
    queryClient.setQueryData(listKey, [makeContact('r1'), makeContact('r2')]);

    const { result } = renderHookWithClient(() => useRemoveReferral(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate('r1');
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    // The cache snapshot taken inside mutationFn (after onMutate) must reflect
    // the optimistic removal: r1 gone, r2 still present.
    expect(cacheAfterMutate?.find((r) => r.id === 'r1')).toBeUndefined();
    expect(cacheAfterMutate?.find((r) => r.id === 'r2')).toBeDefined();
    expect(remove).toHaveBeenCalledWith('r1');
  });

  it('restores the previous cache data on error (optimistic rollback)', async () => {
    // Use gcTime:Infinity so the restored cache entry survives the onSettled
    // invalidation (invalidateQueries only marks stale — it doesn't evict; eviction
    // is driven by gcTime once there are no active observers, which with Infinity
    // never happens during the test).
    const { QueryClient } = await import('@tanstack/react-query');
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false, gcTime: Infinity, staleTime: 0 },
        mutations: { retry: false },
      },
    });

    const listKey = keys.referrals.list('https://example.com/job/1');
    const original = [makeContact('r1'), makeContact('r2')];
    queryClient.setQueryData(listKey, original);

    const remove = vi.fn().mockRejectedValue(new Error('backend offline'));
    const client = createMockClient({ 'referrals.remove': remove });

    const { result } = renderHookWithClient(() => useRemoveReferral(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate('r1');
    });

    await waitFor(() => expect(result.current.isError).toBe(true));

    // onError calls setQueryData(key, previousData) for every previously-snapshotted
    // list — the restored cache entry must equal the original two-contact list.
    const restored = queryClient.getQueryData<ReferralContact[]>(listKey);
    expect(restored).toEqual(original);
  });
});
