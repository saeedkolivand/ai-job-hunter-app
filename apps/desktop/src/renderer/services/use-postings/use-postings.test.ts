import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-postings';
import {
  useInteractions,
  usePersistJob,
  usePostings,
  useUpdatePostingDescription,
} from './use-postings';

describe('use-postings services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('usePersistJob — interactions invalidation', () => {
  /**
   * Real-chain test: uses the REAL usePersistJob + REAL useInteractions hooks
   * with only the IPC client mocked. Verifies that after persistJob settles,
   * the interactions query refetches (i.e. the prefix invalidation hits typed
   * keys like ['postings','interactions','viewed']).
   *
   * This guards against the prior bug where the full key
   * ['postings','interactions',undefined] did NOT match typed queries.
   */
  it('refetches useInteractions("viewed") after persistJob mutates', async () => {
    const listInteractions = vi.fn().mockResolvedValue([]);
    const persistJob = vi.fn().mockResolvedValue(undefined);

    const client = createMockClient({
      'scrape.listInteractions': listInteractions,
      'scrape.persistJob': persistJob,
    });

    const { result } = renderHookWithClient(
      () => ({
        interactions: useInteractions('viewed'),
        persist: usePersistJob(),
      }),
      { client }
    );

    // Wait for the initial interactions query to settle.
    await waitFor(() => expect(result.current.interactions.isSuccess).toBe(true));
    const callCountBefore = listInteractions.mock.calls.length;

    // Trigger the mutation.
    await act(async () => {
      await result.current.persist.mutateAsync({
        job: { id: 'job-1' },
        interactionType: 'viewed',
      });
    });

    // The interactions query for 'viewed' must have re-fired (prefix invalidation).
    await waitFor(() => {
      expect(listInteractions.mock.calls.length).toBeGreaterThan(callCountBefore);
    });

    // The refetch must have been for the 'viewed' type.
    const refetchCall = listInteractions.mock.calls[callCountBefore];
    expect(refetchCall?.[0]).toMatchObject({ interactionType: 'viewed' });
  });
});

describe('useUpdatePostingDescription — postings invalidation', () => {
  /**
   * Real-chain test: verifies that after updateDescription succeeds the postings
   * list query refetches (fix #1 — renderer cache sync after backend persist).
   */
  it('refetches usePostings() after updateDescription mutates', async () => {
    const listPostings = vi.fn().mockResolvedValue([]);
    const updateDescription = vi.fn().mockResolvedValue(undefined);

    const client = createMockClient({
      'scrape.listPostings': listPostings,
      'scrape.updateDescription': updateDescription,
    });

    const { result } = renderHookWithClient(
      () => ({
        postings: usePostings(),
        update: useUpdatePostingDescription(),
      }),
      { client }
    );

    // Wait for the initial postings query to settle.
    await waitFor(() => expect(result.current.postings.isSuccess).toBe(true));
    const callCountBefore = listPostings.mock.calls.length;

    // Trigger the mutation.
    await act(async () => {
      await result.current.update.mutateAsync({ id: 'job-1', description: 'Full text' });
    });

    // The postings query must have re-fired (onSuccess invalidation).
    await waitFor(() => {
      expect(listPostings.mock.calls.length).toBeGreaterThan(callCountBefore);
    });
  });

  it('does not refetch postings when updateDescription rejects', async () => {
    const listPostings = vi.fn().mockResolvedValue([]);
    const updateDescription = vi.fn().mockRejectedValue(new Error('network'));

    const client = createMockClient({
      'scrape.listPostings': listPostings,
      'scrape.updateDescription': updateDescription,
    });

    const { result } = renderHookWithClient(
      () => ({
        postings: usePostings(),
        update: useUpdatePostingDescription(),
      }),
      { client }
    );

    await waitFor(() => expect(result.current.postings.isSuccess).toBe(true));
    const callCountBefore = listPostings.mock.calls.length;

    await act(async () => {
      try {
        await result.current.update.mutateAsync({ id: 'job-1', description: 'Full text' });
      } catch {
        // expected rejection
      }
    });

    // onSuccess must NOT have fired — call count unchanged.
    expect(listPostings.mock.calls.length).toBe(callCountBefore);
  });
});
