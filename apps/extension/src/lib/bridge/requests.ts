/**
 * Request/reply plumbing: the in-flight table correlated by `reqId`, and the
 * per-verb spec (which reply type answers it, how to validate that reply, what
 * a dropped connection resolves to).
 */

import { EXTENSION_MESSAGE_TYPES as T } from '@ajh/shared/extension-protocol';

import { readEnabledFlag } from './guards';
import {
  normalizeAgentCallResult,
  normalizeAgentQueryResult,
  normalizeDocumentExportResult,
  normalizeSettingsResult,
} from './results-agent';
import {
  normalizeAnswerAssistResult,
  normalizeAnswersSaveResult,
  normalizeAnswersSuggestResult,
  normalizeMatchLiveResult,
} from './results-answers';
import {
  normalizeAppliedCheckBatchResult,
  normalizeAppliedCheckResult,
  normalizeImportResult,
  normalizeProfileResult,
  normalizeStatusUpdateResult,
} from './results-tracking';

/** How one verb's reply is recognised, validated, and degraded. */
export interface VerbSpec<R> {
  /** The reply `type` that answers this verb. */
  replyType: string;
  normalize: (payload: unknown) => R;
  /** What the caller sees if the connection drops before the reply. */
  failure: (reason: string) => R;
}

function verb<R>(
  replyType: string,
  normalize: (payload: unknown) => R,
  failure: (reason: string) => R
): VerbSpec<R> {
  return { replyType, normalize, failure };
}

const refusal = (error: string) => ({ ok: false as const, error });

export const VERBS = {
  import: verb(T.importResult, normalizeImportResult, (error) => ({ error })),
  profile: verb(T.profileResult, normalizeProfileResult, (error) => ({ error })),
  applied: verb(T.appliedResult, normalizeAppliedCheckResult, (error) => ({
    found: false,
    error,
  })),
  appliedBatch: verb(T.appliedBatchResult, normalizeAppliedCheckBatchResult, refusal),
  status: verb(T.statusResult, normalizeStatusUpdateResult, refusal),
  // A dropped connection → treat the opt-in as OFF (safe).
  autotrack: verb(T.autotrackResult, readEnabledFlag, () => false),
  autofill: verb(T.autofillResult, readEnabledFlag, () => false),
  answers: verb(T.answersResult, normalizeAnswersSaveResult, refusal),
  suggest: verb(T.answersSuggestResult, normalizeAnswersSuggestResult, refusal),
  match: verb(T.matchResult, normalizeMatchLiveResult, refusal),
  // `settings.get` and `settings.set` answer with the SAME payload.
  settings: verb(T.settingsResult, normalizeSettingsResult, refusal),
  agentQuery: verb(T.agentResult, normalizeAgentQueryResult, (error) => ({
    ok: false as const,
    resource: '',
    error,
  })),
  agentCall: verb(T.agentCallResult, normalizeAgentCallResult, (error) => ({
    dispatched: false as const,
    namespace: '',
    command: '',
    error,
  })),
  documentExport: verb(T.documentResult, normalizeDocumentExportResult, refusal),
  assist: verb(T.answerAssistResult, normalizeAnswerAssistResult, refusal),
};

interface Entry {
  replyType: string;
  resolve: (payload: unknown) => void;
  fail: (reason: string) => void;
}

/** In-flight requests by `reqId`, their timers, and `answer.assist` chunk listeners. */
export class RequestTable {
  private readonly entries = new Map<string, Entry>();
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  /** `answer.assist` streaming-preview callbacks. Registered for EVERY assist
   *  call (even without a caller `onChunk`), because a chunk's arrival also
   *  resets the stall timeout. */
  private readonly chunkListeners = new Map<string, (delta: string) => void>();

  add<R>(reqId: string, spec: VerbSpec<R>, resolve: (result: R) => void): void {
    this.entries.set(reqId, {
      replyType: spec.replyType,
      resolve: (payload) => resolve(spec.normalize(payload)),
      fail: (reason) => resolve(spec.failure(reason)),
    });
  }

  /** (Re)arm the single timer for `reqId`. */
  arm(reqId: string, ms: number, onTimeout: () => void): void {
    this.clearTimer(reqId);
    this.timers.set(reqId, setTimeout(onTimeout, ms));
  }

  clearTimer(reqId: string): void {
    const timer = this.timers.get(reqId);
    if (timer) clearTimeout(timer);
    this.timers.delete(reqId);
  }

  /** Forget `reqId` entirely (entry, timer, chunk listener) without settling it. */
  drop(reqId: string): void {
    this.clearTimer(reqId);
    this.entries.delete(reqId);
    this.chunkListeners.delete(reqId);
  }

  /** Resolve the pending request answered by a `replyType` frame; ignored when none matches. */
  reply(replyType: string, reqId: string, payload: unknown): void {
    const entry = this.entries.get(reqId);
    if (!entry || entry.replyType !== replyType) return;
    this.entries.delete(reqId);
    this.clearTimer(reqId);
    entry.resolve(payload);
  }

  /** The pending `reqId`s awaiting a `replyType` reply (a snapshot). */
  pendingIds(replyType: string): string[] {
    return [...this.entries].filter(([, e]) => e.replyType === replyType).map(([id]) => id);
  }

  /** Drop `reqId` and settle it with its verb's failure result. */
  failOne(reqId: string, reason: string): void {
    const entry = this.entries.get(reqId);
    this.drop(reqId);
    entry?.fail(reason);
  }

  listen(reqId: string, cb: (delta: string) => void): void {
    this.chunkListeners.set(reqId, cb);
  }

  unlisten(reqId: string): void {
    this.chunkListeners.delete(reqId);
  }

  chunk(reqId: string, delta: string): void {
    this.chunkListeners.get(reqId)?.(delta);
  }

  /** Settle every in-flight request with its verb's failure result. */
  failAll(reason: string): void {
    for (const [reqId, entry] of this.entries) {
      const timer = this.timers.get(reqId);
      if (timer) clearTimeout(timer);
      entry.fail(reason);
    }
    this.entries.clear();
    // A dropped connection mid-stream never sends `assist.done` — no more
    // chunks are coming, so retire every listener now.
    this.chunkListeners.clear();
    this.timers.clear();
  }
}
