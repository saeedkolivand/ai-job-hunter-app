/**
 * EmbeddingsSettings — re-index job-outcome notifications.
 *
 * Covers the fix for the bug where a re-index run that failed for every
 * document (or partially) still surfaced as a flat "success" toast:
 *  - `job.completed` with `data.failed > 0` renders the PARTIAL-failure
 *    warning, never the plain success toast.
 *  - `job.completed` with `data.failed === 0` still renders success.
 *  - `job.failed` (a total failure) renders an error toast carrying the
 *    reason string the backend sent as `data`.
 *  - Starting a re-index never fires the `success` notification (only
 *    `info` — "started" and "succeeded" are distinct signals).
 *
 * `useJobEvents` is captured (not invoked) so the test drives the handler
 * directly, mirroring `PrepApplicationPanel.test.tsx`'s established pattern.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { JobEvent } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

// ── i18n stub — encodes the interpolation payload into the returned string so
// a wrong/missing/undefined param (e.g. swapped `failed`/`total`) fails the
// assertions below instead of silently passing (a bare `(k) => k` stub would
// swallow the payload entirely — a real gap a reviewer caught).

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, params?: Record<string, unknown>) => `${k}:${JSON.stringify(params ?? {})}`,
  }),
}));

// ── @ajh/ui — capture the notification calls ──────────────────────────────────

const notifySuccess = vi.fn();
const notifyError = vi.fn();
const notifyWarning = vi.fn();
const notifyInfo = vi.fn();

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({
      open: vi.fn(),
      success: notifySuccess,
      error: notifyError,
      warning: notifyWarning,
      info: notifyInfo,
      destroy: vi.fn(),
    }),
  };
});

// ── @/services — capture the job-event handler; a real document to re-index ──

let jobEventHandler: ((event: JobEvent) => void) | undefined;
const mockReembedMutateAsync = vi.fn().mockResolvedValue({ jobId: 'reembed-1' });

vi.mock('@/services', () => ({
  useEmbeddingStatus: () => ({
    data: {
      active: { provider: 'ollama', model: 'nomic-embed-text', baseUrl: undefined },
      spaces: [],
      documents: { total: 3, indexedInActiveSpace: 1, stale: 2 },
    },
    refetch: vi.fn(),
  }),
  useSetEmbeddingConfig: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useReembedAll: () => ({ mutateAsync: mockReembedMutateAsync, isPending: false }),
  useJobEvents: (cb: (event: JobEvent) => void) => {
    jobEventHandler = cb;
  },
}));

// ── component under test ──────────────────────────────────────────────────────

import { EmbeddingsSettings } from './index';

beforeEach(() => {
  jobEventHandler = undefined;
  mockReembedMutateAsync.mockClear();
  mockReembedMutateAsync.mockResolvedValue({ jobId: 'reembed-1' });
  notifySuccess.mockClear();
  notifyError.mockClear();
  notifyWarning.mockClear();
  notifyInfo.mockClear();
});

async function startReindex() {
  render(<EmbeddingsSettings />);
  await act(async () => {
    fireEvent.click(screen.getByText('settings.embeddings.reindex:{}'));
  });
}

describe('EmbeddingsSettings — reindex job outcome', () => {
  it('starting a reindex fires only the info toast, never success', async () => {
    await startReindex();

    expect(notifyInfo).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'settings.embeddings.reindexStarted:{}' })
    );
    expect(notifySuccess).not.toHaveBeenCalled();
  });

  it('job.completed with failed > 0 renders the partial-failure warning with the real counts, not success', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'reembed-1',
        data: { reembedded: 1, failed: 2, total: 3 },
        ts: 0,
      });
    });

    // The exact payload — a swapped failed/total, or a dropped param, fails this.
    expect(notifyWarning).toHaveBeenCalledWith(
      expect.objectContaining({
        message: 'settings.embeddings.reindexPartial:{"failed":2,"total":3}',
      })
    );
    expect(notifySuccess).not.toHaveBeenCalled();
  });

  it('job.completed with failed === 0 renders success', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'reembed-1',
        data: { reembedded: 3, failed: 0, total: 3 },
        ts: 0,
      });
    });

    expect(notifySuccess).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'settings.embeddings.reindexComplete:{}' })
    );
    expect(notifyWarning).not.toHaveBeenCalled();
  });

  it('job.failed (total failure) renders an error toast carrying the real reason string', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({
        type: 'job.failed',
        jobId: 'reembed-1',
        data: 'Ollama 500 Internal Server Error: the input length exceeds the context length',
        ts: 0,
      });
    });

    // The reason must be the exact backend string, not dropped/undefined.
    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({
        message:
          'settings.embeddings.reindexFailedReason:{"reason":"Ollama 500 Internal Server Error: the input length exceeds the context length"}',
      })
    );
    expect(notifySuccess).not.toHaveBeenCalled();
  });

  it('job.failed with no reason string falls back to the generic incomplete message', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({
        type: 'job.failed',
        jobId: 'reembed-1',
        data: undefined,
        ts: 0,
      });
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'settings.embeddings.reindexIncomplete:{}' })
    );
  });

  it('job.cancelled renders a warning, never error', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({ type: 'job.cancelled', jobId: 'reembed-1', ts: 0 });
    });

    expect(notifyWarning).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'settings.embeddings.reindexIncomplete:{}' })
    );
    expect(notifyError).not.toHaveBeenCalled();
  });

  it('a terminal event delivered via a stale (pre-commit) handler closure still resolves — the ref, not the closed-over state, gates the match', async () => {
    render(<EmbeddingsSettings />);

    // The closure `useJobEvents` registered on the FIRST render — it closes
    // over `reindexJobId === null`, exactly like a real handler still would
    // be if a `job.completed` event raced ahead of the `setReindexJobId`
    // commit. `jobEventHandler` itself gets reassigned to a fresh closure
    // by the second render below, so this reference has to be grabbed now.
    const staleHandler = jobEventHandler;

    await act(async () => {
      fireEvent.click(screen.getByText('settings.embeddings.reindex:{}'));
    });

    // Deliver the terminal event through the STALE closure, not the current
    // one. Before the ref fix this dropped the event (`reindexJobId` inside
    // that closure still reads `null`), leaving the panel stuck
    // "reindexing" forever with no success/failure toast and no refetch.
    act(() => {
      staleHandler?.({
        type: 'job.completed',
        jobId: 'reembed-1',
        data: { reembedded: 3, failed: 0, total: 3 },
        ts: 0,
      });
    });

    expect(notifySuccess).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'settings.embeddings.reindexComplete:{}' })
    );
    // The ref fix must clear the tracked state too, not just let the toast
    // fire — a regression that dropped the `reindexJobIdRef.current = null` /
    // `setReindexJobId(null)` clear (rather than the event match itself)
    // would pass the assertion above while leaving the button stuck showing
    // "reindexing" forever. Its label reverting to the idle "reindex" text
    // is the observable proof the state actually cleared.
    expect(screen.getByText('settings.embeddings.reindex:{}')).toBeInTheDocument();
    expect(screen.queryByText('settings.embeddings.reindexing:{}')).not.toBeInTheDocument();
  });

  it('ignores a job event for a different jobId', async () => {
    await startReindex();

    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'some-other-job',
        data: { reembedded: 0, failed: 3, total: 3 },
        ts: 0,
      });
    });

    expect(notifyWarning).not.toHaveBeenCalled();
    expect(notifySuccess).not.toHaveBeenCalled();
    expect(notifyError).not.toHaveBeenCalled();
  });
});
