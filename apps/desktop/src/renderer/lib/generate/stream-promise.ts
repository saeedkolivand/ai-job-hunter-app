/**
 * Shared streaming-promise scaffold used by all AI generation callers.
 *
 * Both `generation.ts` (resume/cover/etc.) and `resume-ai.ts` (analysis)
 * previously contained an identical `new Promise<string>(…)` block managing:
 *   - `api.ai.onStream` subscription
 *   - abort-signal listener
 *   - 5-minute timeout
 *   - job-status poll fallback
 *   - cleanup on every exit path
 *
 * Extracting to this file eliminates that duplication and unifies:
 *   - The abort-before-register guard (was in `generation.ts` but MISSING in
 *     `resume-ai.ts` — now applied for both callers, fixing the bug where a
 *     cancel that landed during the pipeline request would be silently ignored).
 *   - The poll interval (2 s in `resume-ai.ts`, 3 s in `generation.ts`) — unified
 *     to `JOB_POLL_INTERVAL_MS = 3_000`.
 */

import type { AppClient } from '../app-client';
import { createThinkSplitter } from './think-split';

/** Maximum wall-clock time for a single streamed generation (ms). */
const STREAM_TIMEOUT_MS = 5 * 60 * 1000;

/** Interval between job-status poll ticks (ms). */
const JOB_POLL_INTERVAL_MS = 3_000;

export interface AwaitAiStreamOptions {
  /** Called with each answer token (after think-splitting). */
  onToken?: (tok: string) => void;
  /** Called with each reasoning/thinking token. */
  onThinking?: (tok: string) => void;
  /** AbortSignal — honours both pre-registration and post-registration cancels. */
  signal?: AbortSignal;
  /** Override the poll interval (ms). Defaults to `JOB_POLL_INTERVAL_MS`. */
  pollIntervalMs?: number;
  /** Override the stream timeout (ms). Defaults to `STREAM_TIMEOUT_MS`. */
  timeoutMs?: number;
}

/**
 * Wait for a streamed AI job to complete, collecting the full answer text.
 *
 * The caller is responsible for:
 *  1. Calling `api.ai.generate` / `api.ai.generatePipeline` to enqueue the job.
 *  2. Calling this function with the returned `jobId` AFTER checking the signal
 *     (the abort-before-register guard here handles cancels that arrive between
 *     enqueue and this call).
 *
 * @param api     The `AppClient` instance (from `getClient()`).
 * @param jobId   The job ID returned by the pipeline enqueue call.
 * @param opts    Stream options (callbacks, signal, overrides).
 * @returns       The full answer text (think blocks already stripped).
 */
export function awaitAiStream(
  api: AppClient,
  jobId: string,
  opts: AwaitAiStreamOptions = {}
): Promise<string> {
  const {
    onToken,
    onThinking,
    signal,
    pollIntervalMs = JOB_POLL_INTERVAL_MS,
    timeoutMs = STREAM_TIMEOUT_MS,
  } = opts;

  // ── Abort-before-register guard ──────────────────────────────────────────
  // A cancel that landed while the pipeline request was in flight is already
  // set on the signal by the time we get here. Honour it immediately so the
  // run rejects instead of streaming on silently. This guard was present in
  // generation.ts but was MISSING in resume-ai.ts — it is now enforced for
  // both callers.
  if (signal?.aborted) {
    void api.jobs.cancel(jobId).catch(() => {});
    return Promise.reject(new Error('Generation cancelled'));
  }

  return new Promise<string>((resolve, reject) => {
    let buffer = '';
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let poll: ReturnType<typeof setInterval> | null = null;
    let abortListener: (() => void) | null = null;
    let pollInFlight = false;

    // Local reasoning models embed <think>…</think> inline; the shared splitter
    // separates reasoning from answer text. Cloud providers flag reasoning
    // structurally via the `thinking` chunk flag (handled in the stream handler).
    const splitter = createThinkSplitter(
      (text) => {
        buffer += text;
        onToken?.(text);
      },
      (text) => onThinking?.(text)
    );

    const cleanup = () => {
      if (timeoutId !== null) clearTimeout(timeoutId);
      if (poll !== null) clearInterval(poll);
      if (abortListener && signal) signal.removeEventListener('abort', abortListener);
    };

    const off = api.ai.onStream((chunk: unknown) => {
      const c = chunk as {
        jobId: string;
        delta: string;
        done: boolean;
        error?: { code: string; message: string };
        thinking?: boolean;
      };
      if (c.jobId !== jobId) return;
      if (c.error) {
        off();
        cleanup();
        reject(new Error(c.error.message));
        return;
      }
      if (c.delta) {
        if (c.thinking) {
          // Provider-flagged reasoning token (Anthropic, OpenAI, Gemini, Ollama
          // via the normalised `thinking` chunk flag).
          onThinking?.(c.delta);
        } else {
          // Local model inline <think>…</think> or plain answer token.
          splitter.push(c.delta);
        }
      }
      if (c.done) {
        splitter.flush();
        off();
        cleanup();
        resolve(buffer);
      }
    });

    // Post-registration abort listener.
    if (signal) {
      abortListener = () => {
        off();
        void api.jobs.cancel(jobId).catch(() => {});
        cleanup();
        reject(new Error('Generation cancelled'));
      };
      signal.addEventListener('abort', abortListener);
    }

    // Timeout safety — local LLMs can be slow; cloud calls can queue. On
    // timeout we FAIL the run (never persist truncated output as success) and
    // cancel the backend job so the server-side slot is freed instead of left
    // occupied until the job finishes on its own.
    timeoutId = setTimeout(() => {
      off();
      void api.jobs.cancel(jobId).catch(() => {});
      splitter.flush();
      cleanup();
      reject(new Error('Generation timed out. Please try again.'));
    }, timeoutMs);

    // Job-status poll — fallback for a missed `done` event (empty final delta).
    poll = setInterval(() => {
      if (pollInFlight) return;
      pollInFlight = true;
      void (async () => {
        const job = (await api.jobs.get(jobId).catch(() => null)) as {
          status: string;
          result?: { text: string };
        } | null;
        if (!job) return;
        if (job.status === 'failed' || job.status === 'cancelled') {
          off();
          cleanup();
          reject(new Error(`Generation ${job.status}. Please try again.`));
        } else if (job.status === 'completed') {
          off();
          splitter.flush();
          cleanup();
          // This poll exists because streamed deltas can be dropped, so the
          // buffer is exactly the value that cannot be trusted to be complete.
          // `buffer || job.result?.text` only consulted the persisted text when
          // the buffer was EMPTY — a dropped INTERIOR delta leaves a truncated
          // but truthy prefix, which was then resolved as the finished document
          // and persisted. Take whichever is longer, so a partial stream can
          // never win over the authoritative persisted result (and a missing
          // `result.text` still falls back to whatever did stream).
          const persisted = job.result?.text ?? '';
          resolve(persisted.length > buffer.length ? persisted : buffer);
        }
      })().finally(() => {
        pollInFlight = false;
      });
    }, pollIntervalMs);
  });
}
