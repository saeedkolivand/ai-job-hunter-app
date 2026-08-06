/**
 * useAutoIndex — the auto-indexing trigger.
 *
 * These assert the SPEND-relevant behaviour above all: indexing calls a provider
 * that may bill per token, so "does not run" is as important as "runs".
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, makeQueryClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useAutoIndex } from './use-auto-index';

afterEach(() => {
  vi.restoreAllMocks();
  usePreferencesStore.setState({ autoIndexOnUpload: false });
});

const status = (stale: number, provider = 'ollama', model = 'nomic-embed-text') => ({
  active: { provider, model, baseUrl: null },
  spaces: [],
  documents: { total: stale, indexedInActiveSpace: 0, stale },
});

describe('useAutoIndex', () => {
  it('does nothing when the preference is off, even with stale documents', async () => {
    usePreferencesStore.setState({ autoIndexOnUpload: false });
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: null });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(3)),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    renderHookWithClient(() => useAutoIndex(), { client });

    // Give the status query time to resolve; the call must still never happen.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(indexStaleDocuments).not.toHaveBeenCalled();
  });

  it('indexes when the preference is on and documents are stale', async () => {
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: 'job-1' });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(2)),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    renderHookWithClient(() => useAutoIndex(), { client });

    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(1));
  });

  it('does not run when nothing is stale', async () => {
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: null });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(0)),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    renderHookWithClient(() => useAutoIndex(), { client });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(indexStaleDocuments).not.toHaveBeenCalled();
  });

  it('waits for a configured embedding model instead of failing per refetch', async () => {
    // Onboarding order: the résumé is imported (step 2) BEFORE the AI provider is
    // chosen (step 3), so this state is the normal first-run path, not an edge case.
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: null });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(1, 'ollama', '')),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    renderHookWithClient(() => useAutoIndex(), { client });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(indexStaleDocuments).not.toHaveBeenCalled();
  });

  it('does not start a second run while the first job is still running', async () => {
    // The paid-duplicate guard. `indexStaleDocuments` resolves as soon as the job
    // is SPAWNED, so the status refetch right after it still reports the old
    // stale count — without holding the guard until the job ends, that refetch
    // would start another separately-billed run over the same documents.
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: 'job-1' });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(2)),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    const { rerender } = renderHookWithClient(() => useAutoIndex(), { client });

    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(1));
    rerender();
    rerender();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(indexStaleDocuments).toHaveBeenCalledTimes(1);
  });

  it('indexes a SECOND upload that happens to leave the same stale count', async () => {
    // Regression for the HIGH review finding on #951. The first guard keyed on
    // `provider/model/staleCount` and never reset, so: import one résumé
    // (stale 1 → indexed → stale 0), import another (stale 1 AGAIN) and the
    // second was treated as already handled. Auto-indexing silently died after
    // the very first document. The count identifies a situation, not a batch.
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    let jobHandler: ((e: unknown) => void) | null = null;
    let staleNow = 1;
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: 'job-1' });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockImplementation(async () => status(staleNow)),
      'ai.indexStaleDocuments': indexStaleDocuments,
      'jobs.onEvent': vi.fn((cb: (e: unknown) => void) => {
        jobHandler = cb;
        return () => {};
      }),
    });

    const queryClient = makeQueryClient();
    renderHookWithClient(() => useAutoIndex(), { client, queryClient });
    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(1));

    // The job finishes and the index goes clean.
    staleNow = 0;
    await act(async () => {
      jobHandler?.({ type: 'job.completed', jobId: 'job-1' });
      await new Promise((r) => setTimeout(r, 0));
    });
    await waitFor(() =>
      expect(
        queryClient.getQueryData<{ documents: { stale: number } }>(keys.ai.embeddingStatus)
          ?.documents.stale
      ).toBe(0)
    );

    // A second document arrives, leaving the SAME stale count as the first did.
    staleNow = 1;
    indexStaleDocuments.mockResolvedValue({ jobId: 'job-2' });
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: keys.ai.embeddingStatus });
    });

    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(2), { timeout: 3000 });
  });

  it('does not retry forever when a run fails to reduce the stale count', async () => {
    // The other half of the same guard. Without a per-(space, count) attempt key
    // a run that indexes nothing re-triggers the instant it ends — an unbounded
    // loop of paid provider calls.
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    let jobHandler: ((e: unknown) => void) | null = null;
    const indexStaleDocuments = vi.fn().mockResolvedValue({ jobId: 'job-1' });
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(2)),
      'ai.indexStaleDocuments': indexStaleDocuments,
      'jobs.onEvent': vi.fn((cb: (e: unknown) => void) => {
        jobHandler = cb;
        return () => {};
      }),
    });

    const { rerender } = renderHookWithClient(() => useAutoIndex(), { client });
    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(1));

    // Job ends with the count unchanged (every document failed to embed).
    await act(async () => {
      jobHandler?.({ type: 'job.failed', jobId: 'job-1' });
      await new Promise((r) => setTimeout(r, 0));
    });
    rerender();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(indexStaleDocuments).toHaveBeenCalledTimes(1);
  });
});
