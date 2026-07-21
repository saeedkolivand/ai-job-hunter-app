import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
} from '@/test-support';

import { usePostings } from '../use-postings';
import * as mod from './use-dedup';
import { useMarkNotDuplicate } from './use-dedup';

describe('use-dedup services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useMarkNotDuplicate — postings invalidation', () => {
  /**
   * Real-chain test: after a split settles, the postings list query refetches
   * so the ungrouped rows re-render with the backend's recomputed annotations.
   */
  it('refetches usePostings() after markNotDuplicate mutates', async () => {
    const listPostings = vi.fn().mockResolvedValue([]);
    const markNotDuplicate = vi.fn().mockResolvedValue({ success: true });

    const client = createMockClient({
      'scrape.listPostings': listPostings,
      'dedup.markNotDuplicate': markNotDuplicate,
    });

    const { result } = renderHookWithClient(
      () => ({
        postings: usePostings(),
        split: useMarkNotDuplicate(),
      }),
      { client }
    );

    await waitFor(() => expect(result.current.postings.isSuccess).toBe(true));
    const callCountBefore = listPostings.mock.calls.length;

    await act(async () => {
      await result.current.split.mutateAsync({ memberKey: 'k1', otherKeys: ['k2'] });
    });

    await waitFor(() => {
      expect(listPostings.mock.calls.length).toBeGreaterThan(callCountBefore);
    });
    expect(markNotDuplicate).toHaveBeenCalledWith({ memberKey: 'k1', otherKeys: ['k2'] });
  });

  it('does not refetch postings when markNotDuplicate rejects', async () => {
    const listPostings = vi.fn().mockResolvedValue([]);
    const markNotDuplicate = vi.fn().mockRejectedValue(new Error('boom'));

    const client = createMockClient({
      'scrape.listPostings': listPostings,
      'dedup.markNotDuplicate': markNotDuplicate,
    });

    const { result } = renderHookWithClient(
      () => ({
        postings: usePostings(),
        split: useMarkNotDuplicate(),
      }),
      { client }
    );

    await waitFor(() => expect(result.current.postings.isSuccess).toBe(true));
    const callCountBefore = listPostings.mock.calls.length;

    await act(async () => {
      try {
        await result.current.split.mutateAsync({ memberKey: 'k1', otherKeys: ['k2'] });
      } catch {
        // expected rejection
      }
    });

    // onSuccess must NOT have fired — call count unchanged.
    expect(listPostings.mock.calls.length).toBe(callCountBefore);
  });

  /**
   * The real Tauri failure mode: the command RESOLVES `json!({"error": ...})` as
   * a value (it never rejects), so a naive mutationFn would fire `onSuccess` and
   * show a false success toast. The hook must narrow the `{ error }` union and
   * throw — so no invalidation runs.
   */
  it('errors and does NOT invalidate when the command RESOLVES an { error } payload', async () => {
    const listPostings = vi.fn().mockResolvedValue([]);
    const markNotDuplicate = vi.fn().mockResolvedValue({ error: 'boom' });

    const client = createMockClient({
      'scrape.listPostings': listPostings,
      'dedup.markNotDuplicate': markNotDuplicate,
    });

    const { result } = renderHookWithClient(
      () => ({ postings: usePostings(), split: useMarkNotDuplicate() }),
      { client }
    );

    await waitFor(() => expect(result.current.postings.isSuccess).toBe(true));
    const callCountBefore = listPostings.mock.calls.length;

    await act(async () => {
      await expect(
        result.current.split.mutateAsync({ memberKey: 'k1', otherKeys: ['k2'] })
      ).rejects.toThrow('boom');
    });

    // onSuccess never fired → no postings refetch.
    expect(listPostings.mock.calls.length).toBe(callCountBefore);
  });

  /**
   * Both invalidations matter: a split recomputes the autopilot found-jobs too
   * (ADR-029 §h), so a dropped `keys.autopilot.all` invalidate would leave the
   * AutopilotCard rows stale. Spy on the QueryClient to assert BOTH keys fire.
   */
  it('invalidates BOTH the postings and autopilot query keys on success', async () => {
    const markNotDuplicate = vi.fn().mockResolvedValue({ success: true });
    const client = createMockClient({ 'dedup.markNotDuplicate': markNotDuplicate });
    const queryClient = makeQueryClient();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useMarkNotDuplicate(), { client, queryClient });

    await act(async () => {
      await result.current.mutateAsync({ memberKey: 'k1', otherKeys: ['k2'] });
    });

    const invalidatedKeys = invalidateSpy.mock.calls.map(
      (call) => (call[0] as { queryKey?: unknown } | undefined)?.queryKey
    );
    expect(invalidatedKeys).toContainEqual(['postings']);
    expect(invalidatedKeys).toContainEqual(['autopilot']);
  });
});
