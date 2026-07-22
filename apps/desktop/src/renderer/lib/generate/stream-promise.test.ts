/**
 * awaitAiStream — job-status poll fallback.
 *
 * The poll exists precisely because streamed deltas can be dropped (a missed
 * `done`, or a lost chunk mid-stream), so what it resolves must not trust the
 * streamed buffer over the persisted job result.
 *
 * The mock below mirrors the REAL backend contract: on a streamed generation's
 * completion, `finish` (apps/desktop/src-tauri/src/commands/ai_provider/stream.rs)
 * persists `result = { done: true, text }`, where `text` is the full completed
 * answer with inline `<think>…</think>` reasoning already stripped backend-side
 * (`strip_think_blocks`, mirroring `think-split.ts`). The renderer trusts that
 * text verbatim — it does NOT re-strip — so the no-markup guarantee lives in the
 * backend. Before this backend change the result was `{ done: true }` with no
 * `text`, which made this poll fallback a runtime no-op (PR #802 review finding).
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

/** A minimal `AppClient` whose stream can be driven by hand and whose
 *  `jobs.get` reports `completed` with the REAL persisted result shape the
 *  backend produces: `{ done: true, text }` (see `finish` in stream.rs).
 *  `persisted === undefined` models an older/other backend whose completed
 *  result carries `done` but no `text` — the poll must then fall back to the
 *  streamed buffer. */
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
            ? { status: 'completed', result: { done: true } }
            : { status: 'completed', result: { done: true, text: persisted } }
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
    // The completed result carries `done` but no `text` (older/other backend) —
    // the streamed answer is all there is, so the buffer must win.
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

  it('recovered persisted text never contains reasoning markup', async () => {
    // The local model reasoned inline (`<think>…</think>`), but the backend's
    // `finish` strips it before persisting `result.text` (see `strip_think_blocks`
    // in stream.rs), so the poll's longer-wins branch resolves a clean document.
    // This pins the end-to-end guarantee: because the persisted contract is
    // think-stripped, a persisted result can never leak reasoning markup into the
    // resolved text — even though the renderer trusts `result.text` verbatim.
    const clean = 'Dear hiring manager, I am a strong fit for this role. Sincerely, Jane.';
    const { api, push } = makeApi(clean);

    const promise = awaitAiStream(api, 'job-4', { pollIntervalMs: 1 });
    // Only a truncated prefix streamed; no `done` chunk ever lands.
    push({ jobId: 'job-4', delta: 'Dear hiring manager, ', done: false });

    const resolved = await promise;
    expect(resolved).toBe(clean);
    expect(resolved).not.toContain('<think>');
    expect(resolved).not.toContain('</think>');
  });
});
