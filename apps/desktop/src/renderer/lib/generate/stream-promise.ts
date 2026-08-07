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

/**
 * Baseline wall-clock budget for a single streamed generation (ms), before
 * {@link EFFORT_TIMEOUT_MULTIPLIER} scaling. MUST match the Rust
 * `timeouts::STREAM` baseline (`apps/desktop/src-tauri/src/commands/ai_provider/timeouts.rs`)
 * — see {@link computeStreamTimeoutMs}'s doc for why the two can't literally
 * share code and are pinned by a test instead.
 */
const STREAM_TIMEOUT_MS = 5 * 60 * 1000;

/** Interval between job-status poll ticks (ms). */
const JOB_POLL_INTERVAL_MS = 3_000;

/**
 * Reasoning-effort → {@link STREAM_TIMEOUT_MS} multiplier. MUST mirror Rust's
 * `effort_multiplier` (`timeouts.rs`) exactly — same closed vocabulary, same
 * numbers — or the two deadlines could disagree about which effort level
 * warrants extra time. Rust and TS can't share a literal table across the IPC
 * boundary, so this is duplicated deliberately; `stream-promise.test.ts`'s
 * `computeStreamTimeoutMs` tests pin the one invariant that actually matters
 * (the renderer timeout for a given effort always exceeds the Rust backend's
 * own scaled deadline for that SAME effort, by at least {@link OUTER_BOUND_MARGIN_MS}),
 * so a future edit to either table that breaks the relationship fails a test
 * instead of silently drifting.
 */
const EFFORT_TIMEOUT_MULTIPLIER: Record<string, number> = {
  // Ascending tier order — `max` is the TOP tier, above `xhigh`, per Anthropic's
  // effort docs and OpenAI's `reasoning_effort`. Not alphabetical, and it reads
  // wrong at a glance: an earlier version had `max` below `xhigh`, giving the
  // highest-effort runs the shortest deadline. Mirrors `effort_multiplier` in
  // `apps/desktop/src-tauri/src/commands/ai_provider/timeouts.rs` — keep both in
  // this order.
  medium: 1.5,
  high: 2.0,
  xhigh: 2.5,
  max: 3.0,
};

/**
 * Safety margin (ms) the renderer timeout adds on top of the backend's own
 * `timeouts::stream_deadline` for the same effort — the backend deadline must
 * always fire FIRST (it returns an actionable provider error; a renderer
 * timeout is a generic "Generation timed out"), so the renderer's own bound
 * has to strictly exceed it, not merely match it.
 */
const OUTER_BOUND_MARGIN_MS = 30_000;

/**
 * The renderer-side stream timeout for a given `effort` string (the SAME
 * value threaded to the backend as `AiGenerateRequest.effort`) — the outer
 * bound the backend's own `timeouts::stream_deadline` must always fire
 * before. An unrecognized/absent effort gets multiplier 1 (the baseline),
 * matching Rust's fallback for the same case.
 */
export function computeStreamTimeoutMs(effort?: string): number {
  const multiplier = (effort ? EFFORT_TIMEOUT_MULTIPLIER[effort] : undefined) ?? 1;
  return Math.round(STREAM_TIMEOUT_MS * multiplier) + OUTER_BOUND_MARGIN_MS;
}

/**
 * Rejection message for a completion that resolved with no usable content
 * (whitespace-only counts as empty). Worded to match the export path's own
 * empty-document rejection (`apps/desktop/src-tauri/src/export/commands/mod.rs`)
 * — the user needs to know generation produced nothing, and that retrying is
 * the fix, not that something crashed.
 */
const EMPTY_GENERATION_MESSAGE = 'Generation produced no content. Please try again.';

export interface AwaitAiStreamOptions {
  /** Called with each answer token (after think-splitting). */
  onToken?: (tok: string) => void;
  /** Called with each reasoning/thinking token. */
  onThinking?: (tok: string) => void;
  /** AbortSignal — honours both pre-registration and post-registration cancels. */
  signal?: AbortSignal;
  /** Override the poll interval (ms). Defaults to `JOB_POLL_INTERVAL_MS`. */
  pollIntervalMs?: number;
  /** Override the stream timeout (ms) outright. Defaults to
   *  `computeStreamTimeoutMs(effort)` — prefer passing `effort` over this. */
  timeoutMs?: number;
  /** The SAME reasoning-effort value sent to the backend as
   *  `AiGenerateRequest.effort` — sizes the default `timeoutMs` via
   *  `computeStreamTimeoutMs` so a high-effort generation isn't killed
   *  client-side while the backend is still legitimately streaming. */
  effort?: string;
  /** Active provider — diagnostic only, logged (never generated content) if the
   *  completion resolves empty. */
  provider?: string;
  /** Active model — diagnostic only, same as `provider`. */
  model?: string;
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
    timeoutMs = computeStreamTimeoutMs(opts.effort),
    provider,
    model,
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
        // A `done` chunk with no usable content (empty, or whitespace-only) is
        // NOT a successful generation — resolving it as one is what let an
        // empty document sail through persist/export with no error shown.
        if (!buffer.trim()) {
          console.warn('[awaitAiStream] empty completion', { jobId, provider, model });
          reject(new Error(EMPTY_GENERATION_MESSAGE));
          return;
        }
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
          const finalText = persisted.length > buffer.length ? persisted : buffer;
          // Same empty guard as the `done`-chunk path: neither the streamed
          // buffer nor the persisted job result has content — fail instead of
          // resolving '' as a finished document.
          if (!finalText.trim()) {
            console.warn('[awaitAiStream] empty completion (poll)', { jobId, provider, model });
            reject(new Error(EMPTY_GENERATION_MESSAGE));
            return;
          }
          resolve(finalText);
        }
      })().finally(() => {
        pollInFlight = false;
      });
    }, pollIntervalMs);
  });
}
