/**
 * useAutoIndex — the auto-indexing trigger.
 *
 * These assert the SPEND-relevant behaviour above all: indexing calls a provider
 * that may bill per token, so "does not run" is as important as "runs".
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

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

  it('does not queue a second run for the same situation', async () => {
    // The paid-duplicate guard: the status query refetches, and without the
    // attempt key each refetch would start another index job over the same
    // documents.
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

  it('survives a failing index call — matching still embeds lazily', async () => {
    usePreferencesStore.setState({ autoIndexOnUpload: true });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const indexStaleDocuments = vi.fn().mockRejectedValue(new Error('provider down'));
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(status(1)),
      'ai.indexStaleDocuments': indexStaleDocuments,
    });

    renderHookWithClient(() => useAutoIndex(), { client });

    await waitFor(() => expect(indexStaleDocuments).toHaveBeenCalledTimes(1));
    // No unhandled rejection, and the raw provider message never reaches the log.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(JSON.stringify(vi.mocked(console.warn).mock.calls)).not.toContain('provider down');
  });
});
