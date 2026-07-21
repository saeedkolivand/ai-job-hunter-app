/**
 * awaitAiStream — job-status poll fallback.
 *
 * The poll exists precisely because streamed deltas can be dropped (a missed
 * `done`, or a lost chunk mid-stream), so what it resolves must not trust the
 * streamed buffer over the persisted job result.
 */

import { describe, expect, it, vi } from 'vitest';

import type { AppClient } from '../app-client';
import { awaitAiStream } from './stream-promise';

interface StreamChunk {
  jobId: string;
  delta: string;
  done: boolean;
  thinking?: boolean;
}

/** A minimal `AppClient` whose stream can be driven by hand and whose job
 *  status reports `completed` with the given persisted text. */
function makeApi(persisted: string | undefined) {
  let onChunk: ((chunk: StreamChunk) => void) | null = null;
  const api = {
    ai: {
      onStream: (cb: (chunk: StreamChunk) => void) => {
        onChunk = cb;
        return () => {
          onChunk = null;
        };
      },
    },
    jobs: {
      get: vi
        .fn()
        .mockResolvedValue(
          persisted === undefined
            ? { status: 'completed' }
            : { status: 'completed', result: { text: persisted } }
        ),
      cancel: vi.fn().mockResolvedValue(undefined),
    },
  } as unknown as AppClient;

  return { api, push: (chunk: StreamChunk) => onChunk?.(chunk) };
}

describe('awaitAiStream — poll fallback', () => {
  it('resolves the persisted result when the streamed buffer is a truncated prefix', async () => {
    const full = 'Dear hiring manager, I am writing to apply for the role. Sincerely, Jane.';
    const { api, push } = makeApi(full);

    const promise = awaitAiStream(api, 'job-1', { pollIntervalMs: 1 });
    // An interior delta was dropped: what arrived is a truthy but incomplete
    // prefix, and no `done` chunk ever lands — the poll has to finish the job.
    push({ jobId: 'job-1', delta: 'Dear hiring manager, ', done: false });

    await expect(promise).resolves.toBe(full);
  });

  it('keeps the streamed buffer when it is the longer of the two', async () => {
    // The persisted text is missing/empty (older backend, or a result shape
    // without `text`) — the streamed answer is all there is.
    const { api, push } = makeApi(undefined);

    const promise = awaitAiStream(api, 'job-2', { pollIntervalMs: 1 });
    push({ jobId: 'job-2', delta: 'streamed answer', done: false });

    await expect(promise).resolves.toBe('streamed answer');
  });

  it('still resolves the buffer immediately on a done chunk (no poll involved)', async () => {
    const { api, push } = makeApi('ignored — the done path never polls');

    const promise = awaitAiStream(api, 'job-3', { pollIntervalMs: 10_000 });
    push({ jobId: 'job-3', delta: 'complete answer', done: true });

    await expect(promise).resolves.toBe('complete answer');
  });
});
