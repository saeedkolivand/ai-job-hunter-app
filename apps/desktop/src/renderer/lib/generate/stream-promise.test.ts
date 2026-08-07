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
import { awaitAiStream, computeStreamTimeoutMs } from './stream-promise';

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

  it('still resolves a short-but-real generation (no invented minimum length)', async () => {
    const { api, push } = makeApi('ignored — the done path never polls');

    const promise = awaitAiStream(api, 'job-short', { pollIntervalMs: 10_000 });
    push({ jobId: 'job-short', delta: 'Yes.', done: true });

    await expect(promise).resolves.toBe('Yes.');
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

describe('awaitAiStream — empty completion rejects (both resolve paths)', () => {
  // The generation pipeline treated ANY resolve as success, including an empty
  // one — an empty document was then silently persisted and shown with no
  // error. Both places `awaitAiStream` can settle must instead reject.

  it('rejects on the `done`-chunk path when no content ever streamed', async () => {
    const { api, push } = makeApi(undefined);

    const promise = awaitAiStream(api, 'job-empty-done', { pollIntervalMs: 10_000 });
    push({ jobId: 'job-empty-done', delta: '', done: true });

    await expect(promise).rejects.toThrow('Generation produced no content. Please try again.');
  });

  it('rejects on the `done`-chunk path when the stream emitted only whitespace', async () => {
    const { api, push } = makeApi(undefined);

    const promise = awaitAiStream(api, 'job-whitespace-done', { pollIntervalMs: 10_000 });
    push({ jobId: 'job-whitespace-done', delta: '  \n\t', done: false });
    push({ jobId: 'job-whitespace-done', delta: '', done: true });

    await expect(promise).rejects.toThrow('Generation produced no content. Please try again.');
  });

  it('rejects on the poll-fallback path when both the buffer and the persisted result are empty', async () => {
    // `text: ''` — the completed job's persisted result really did carry no
    // content, and no `done` chunk (nor any delta) ever streamed either.
    const { api } = makeApi('');

    const promise = awaitAiStream(api, 'job-empty-poll', { pollIntervalMs: 1 });

    await expect(promise).rejects.toThrow('Generation produced no content. Please try again.');
  });
});

describe('computeStreamTimeoutMs — effort scaling', () => {
  const BASELINE_MS = 5 * 60 * 1000 + 30_000; // STREAM_TIMEOUT_MS + OUTER_BOUND_MARGIN_MS

  it('uses the flat baseline (+ margin) for no, low-tier, or unrecognized effort', () => {
    for (const effort of [undefined, 'minimal', 'low', 'bogus-provider-string']) {
      expect(computeStreamTimeoutMs(effort)).toBe(BASELINE_MS);
    }
  });

  it('scales up, strictly non-decreasing, by effort tier', () => {
    // Vendors' ascending tier order: `max` is the TOP tier, above `xhigh`.
    const tiers = [undefined, 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];
    let prev = 0;
    for (const effort of tiers) {
      const ms = computeStreamTimeoutMs(effort);
      expect(ms).toBeGreaterThanOrEqual(prev);
      prev = ms;
    }
    // And the top tier must actually be LARGER than the baseline, not just
    // equal — otherwise "scales with effort" would be vacuously true.
    expect(computeStreamTimeoutMs('max')).toBeGreaterThan(BASELINE_MS);
  });

  // Cross-language relationship (ADR-style pin, see the task this closed): Rust
  // and TS can't literally share the effort→multiplier table across the IPC
  // boundary, so this hardcodes the SAME schedule `timeouts::effort_multiplier`
  // (apps/desktop/src-tauri/src/commands/ai_provider/timeouts.rs) encodes and
  // asserts the one invariant that actually matters — the renderer timeout for
  // a given effort must always exceed the backend's own scaled deadline for
  // that SAME effort, so the backend (an actionable provider error) fires
  // first, never the renderer's generic timeout. A change to either table that
  // breaks this relationship must fail HERE, not surface as a support report.
  it('stays strictly above the mirrored Rust backend deadline for every known effort level', () => {
    const RUST_STREAM_BASELINE_MS = 300 * 1000; // timeouts::STREAM
    const rustEffortMultiplier: Record<string, number> = {
      medium: 1.5,
      high: 2.0,
      xhigh: 2.5,
      max: 3.0,
    };
    for (const effort of [undefined, 'minimal', 'low', 'medium', 'high', 'xhigh', 'max']) {
      const multiplier = (effort ? rustEffortMultiplier[effort] : undefined) ?? 1;
      const backendMs = RUST_STREAM_BASELINE_MS * multiplier;
      const rendererMs = computeStreamTimeoutMs(effort);
      expect(rendererMs).toBeGreaterThan(backendMs);
    }
  });
});
