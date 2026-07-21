import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

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
});
