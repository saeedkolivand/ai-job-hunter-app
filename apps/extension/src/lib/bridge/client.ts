/**
 * The bridge verbs: one thin wrapper per request type over a single
 * request/reply path (`request`). Every verb is gated on the AUTHENTICATED
 * session (the mutual handshake completed), never on transport liveness or on
 * `phase === 'connected'` alone (reached with no handshake when no token is stored).
 */

import {
  EXTENSION_MESSAGE_TYPES as T,
  type ExtensionAgentCallResult,
  type ExtensionAgentQueryResult,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestResult,
  type ExtensionAppliedCheckBatchResult,
  type ExtensionAppliedCheckResult,
  type ExtensionDocumentExportRequest,
  type ExtensionDocumentExportResult,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveRequest,
  type ExtensionMatchLiveResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
  type ExtensionSettingsKey,
  type ExtensionSettingsResult,
  type ExtensionStatusUpdateResult,
} from '@ajh/shared/extension-protocol';

import { BridgeConnection } from './connection';
import { ASSIST_STALL_TIMEOUT_MS, REQUEST_TIMEOUT_MS } from './constants';
import { VERBS, type VerbSpec } from './requests';
import type { BridgeTransport } from './transport';

const NOT_REACHABLE = 'Desktop app not reachable. Is AI Job Hunter running?';
const TIMED_OUT = 'Timed out waiting for the desktop app to respond.';

export class BridgeClient extends BridgeConnection {
  /**
   * Connect, then hand back the transport — but ONLY for an authenticated
   * session. A non-null `this.transport` does NOT mean the peer is verified:
   * `attach()` sets it before the handshake checks the peer's `serverProof`.
   * Only `'connected'` AND `authenticated` mean the mutual handshake completed
   * (`'connected'` alone is also reached with NO handshake when no token is
   * stored), so this is the seam
   * that stops the active-tab DOM (or the Contact Profile, the highest
   * sensitivity payload this client sends) from ever reaching an
   * unverified/rogue peer.
   */
  private async session(): Promise<BridgeTransport> {
    await this.ensureConnected();
    if (!this.hasAuthenticatedSession() || !this.transport) throw new Error(NOT_REACHABLE);
    return this.transport;
  }

  /** The mutual handshake completed on the live transport (never true for the no-token attach). */
  private hasAuthenticatedSession(): boolean {
    return this.phase === 'connected' && this.authenticated;
  }

  /**
   * Send one request and resolve with its validated reply (see {@link VERBS}).
   * Rejects only on no connection / timeout / send failure — a well-formed
   * `ok:false` reply is passed through, never folded away.
   */
  private async request<R>(
    spec: VerbSpec<R>,
    type: ExtensionMessageType,
    payload: unknown
  ): Promise<R> {
    const transport = await this.session();
    const reqId = crypto.randomUUID();
    return new Promise<R>((resolve, reject) => {
      this.requests.arm(reqId, REQUEST_TIMEOUT_MS, () => {
        this.requests.drop(reqId);
        reject(new Error(TIMED_OUT));
      });
      this.requests.add(reqId, spec, resolve);
      try {
        transport.send({ type, reqId, payload });
      } catch (err) {
        this.requests.drop(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /** Send an `import.request`; the frame carries NO token (the socket is already authenticated). */
  importJob(payload: ExtensionImportRequest): Promise<ExtensionImportResult> {
    return this.request(VERBS.import, T.importRequest, payload);
  }

  /**
   * `profile.get` — the contact profile for assisted autofill, or `{ error }`
   * when the desktop refuses (autofill opt-in off) or the reply is malformed.
   * Returned to the caller transiently, never stored by this client.
   */
  getProfile(): Promise<ExtensionProfileResult> {
    return this.request(VERBS.profile, T.profileGet, null);
  }

  /** `applied.check` — whether an Application already exists for `url` (read-only, never blocks the import controls). */
  checkApplied(url: string): Promise<ExtensionAppliedCheckResult> {
    return this.request(VERBS.applied, T.appliedCheck, { url });
  }

  /**
   * `applied.check.batch` (results-page stamps). UNLIKE `checkApplied`, a
   * well-formed `ok:false` reply is NOT folded away: the caller degrades an
   * over-cap/throttle/malformed refusal to "no stamps" itself.
   */
  checkAppliedBatch(urls: string[]): Promise<ExtensionAppliedCheckBatchResult> {
    return this.request(VERBS.appliedBatch, T.appliedCheckBatch, { urls });
  }

  /**
   * `status.update { url, to: 'applied' }`. `auto: true` marks the automated
   * auto-track write; the desktop re-gates it on that opt-in. Omitted for the
   * ordinary popup click (ungated).
   */
  updateStatus(url: string, auto = false): Promise<ExtensionStatusUpdateResult> {
    const payload = auto ? { url, to: 'applied', auto: true } : { url, to: 'applied' };
    return this.request(VERBS.status, T.statusUpdate, payload);
  }

  /**
   * Read the desktop-enforced auto-track opt-in — `true` only when the desktop
   * replies `{ enabled: true }`. UNLIKE the other verbs this NEVER rejects: any
   * failure (not connected, timeout, send error, a malformed reply) degrades to
   * `false` (OFF, the safe default), because callers treat "unknown" exactly as
   * "off" — never arm, never auto-write, when the opt-in can't be confirmed.
   */
  autotrackEnabled(): Promise<boolean> {
    return this.request(VERBS.autotrack, T.autotrackCheck, null).catch(() => false);
  }

  /**
   * Read the desktop-enforced assisted-autofill opt-in. Mirrors
   * {@link autotrackEnabled}: NEVER rejects, degrades to `false`. The desktop
   * still enforces the real gate on `answers.suggest` regardless of this read.
   */
  autofillEnabled(): Promise<boolean> {
    return this.request(VERBS.autofill, T.autofillCheck, null).catch(() => false);
  }

  /** `answers.save { url, answers }`; `auto` mirrors {@link updateStatus}'s own param (save-answers-on-submit). */
  saveAnswers(
    url: string,
    answers: ExtensionAnswerPair[],
    auto = false
  ): Promise<ExtensionAnswersSaveResult> {
    const payload = auto ? { url, answers, auto: true } : { url, answers };
    return this.request(VERBS.answers, T.answersSave, payload);
  }

  /** `answers.suggest { questions }` — the desktop's fuzzy-matched suggestions (or a refusal). */
  suggestAnswers(questions: string[]): Promise<ExtensionAnswersSuggestResult> {
    return this.request(VERBS.suggest, T.answersSuggest, { questions });
  }

  /** `match.live { url, html }` (the user-clicked "Check fit"). */
  matchLive(payload: ExtensionMatchLiveRequest): Promise<ExtensionMatchLiveResult> {
    return this.request(VERBS.match, T.matchLive, payload);
  }

  /** `settings.get` — the current `{ autofill, aiAssist, autotrack, saveAnswersOnSubmit }` values, or a refusal. */
  settingsGet(): Promise<ExtensionSettingsResult> {
    return this.request(VERBS.settings, T.settingsGet, {});
  }

  /** `settings.set { key, enabled }` — the full new settings, or a refusal the caller must roll its optimistic toggle back on. */
  settingsSet(key: ExtensionSettingsKey, enabled: boolean): Promise<ExtensionSettingsResult> {
    return this.request(VERBS.settings, T.settingsSet, { key, enabled });
  }

  /**
   * `agent.query { resource, params? }` (extension read tier) — the curated
   * resource's data, or a refusal (Autofill opt-in off, the 256 KiB reply cap,
   * a throttle, or an unknown/malformed resource). Resource-specific fields sit
   * at the payload's TOP level (matches the Rust readers AND the CLI's own wire
   * builder); `resource` spreads LAST so a colliding `params.resource` key can
   * never override the resource this call actually named.
   */
  agentQuery(
    resource: string,
    params?: Record<string, unknown>
  ): Promise<ExtensionAgentQueryResult> {
    return this.request(VERBS.agentQuery, T.agentQuery, { ...params, resource });
  }

  /**
   * `agent.call { namespace, command, input? }` (extension read tier).
   * `command` here is the full `<namespace>:<command>` path — split on the first
   * `:` to build the wire frame's flat fields (matches the Rust `payload_target`
   * reader). ONLY `Effect::Read` policy rows ever dispatch for this caller;
   * anything else comes back `dispatched:false` naming the tier.
   */
  agentCall(command: string, args?: unknown): Promise<ExtensionAgentCallResult> {
    const colonIdx = command.indexOf(':');
    const namespace = colonIdx === -1 ? command : command.slice(0, colonIdx);
    const commandName = colonIdx === -1 ? '' : command.slice(colonIdx + 1);
    return this.request(VERBS.agentCall, T.agentCall, {
      namespace,
      command: commandName,
      input: args === undefined ? {} : args,
    });
  }

  /**
   * `document.export` (documents into ATS) — the rendered document as base64
   * bytes, or a refusal. This client decodes NOTHING beyond validating
   * `dataEncoding === 'base64'`; turning `data` into bytes/text is the caller's job.
   */
  documentExport(payload: ExtensionDocumentExportRequest): Promise<ExtensionDocumentExportResult> {
    return this.request(VERBS.documentExport, T.documentExport, payload);
  }

  /**
   * `answer.assist` (the user-clicked "Help me answer…", or a rewrite) — the
   * first BILLABLE-AI verb on the bridge. `payload` is forwarded VERBATIM: the
   * desktop validates it server-side.
   *
   * The desktop STREAMS the answer: zero or more `assist.chunk { delta }` frames
   * arrive before the terminal `answer.assist.result` resolves this promise.
   * Each delta is forwarded to `onChunk` (best-effort live preview); the caller
   * (background/answer-assist.ts) ACCUMULATES the running text — this client holds no
   * cross-eviction buffer.
   *
   * Unlike every other verb's flat {@link REQUEST_TIMEOUT_MS}, this promise is
   * guarded by a STALL timer ({@link ASSIST_STALL_TIMEOUT_MS}) reset on every
   * chunk. Only genuine silence for that long fires the timeout, which also
   * sends `assist.cancel` (so the desktop stops streaming/charging, not just
   * this client giving up) before rejecting.
   *
   * At most ONE stream is ever active client-side: a call made while a PRIOR
   * one is still pending retires that older request FIRST (its chunk listener
   * is dropped — `onChunk` only receives a bare `delta`, with no `reqId` to
   * guard against stale chunks — and its promise settles `{ok:false}` rather
   * than dangling until the stall timeout).
   */
  async answerAssist(
    payload: ExtensionAnswerAssistRequest,
    onChunk?: (delta: string) => void
  ): Promise<ExtensionAnswerAssistResult> {
    this.supersedeAnyPendingAssist();
    const transport = await this.session();
    const reqId = crypto.randomUUID();

    return new Promise<ExtensionAnswerAssistResult>((resolve, reject) => {
      // Re-armed on every chunk — a stall is "no activity at all for
      // ASSIST_STALL_TIMEOUT_MS", not "the whole request took longer than it".
      const armStallTimer = (): void =>
        this.requests.arm(reqId, ASSIST_STALL_TIMEOUT_MS, () => {
          this.requests.drop(reqId);
          // Stop the desktop's stream (and its provider spend) too — a stalled
          // client-side promise must not leave an orphaned, still-billing
          // generation running server-side.
          this.cancelAssist(reqId);
          reject(new Error(TIMED_OUT));
        });

      // Always registered (even with no `onChunk`) — every chunk must reset the
      // stall clock regardless of whether the caller wants the live preview.
      this.requests.listen(reqId, (delta) => {
        armStallTimer();
        onChunk?.(delta);
      });
      armStallTimer();
      this.requests.add(reqId, VERBS.assist, (result) => {
        this.requests.unlisten(reqId);
        resolve(result);
      });

      try {
        transport.send({ type: T.answerAssist, reqId, payload });
      } catch (err) {
        this.requests.drop(reqId);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  /**
   * Retire every still-pending `answerAssist` call: drop its chunk listener +
   * stall timer, best-effort send its `assist.cancel`, and SETTLE its promise
   * (`{ok:false}`) — without that, a superseded request's own `onChunk` closure
   * stays registered and would keep mutating whatever shared buffer its caller
   * accumulates into. A no-op when nothing is pending.
   */
  private supersedeAnyPendingAssist(): void {
    for (const staleReqId of this.requests.pendingIds(VERBS.assist.replyType)) {
      this.cancelAssist(staleReqId);
      this.requests.failOne(staleReqId, 'Superseded by a newer request.');
    }
  }

  /**
   * Send an `assist.cancel` for a still-streaming `answer.assist` `reqId`.
   * Best-effort — a send failure or no connection is silently ignored (there is
   * no reply to correlate). Also drops the local chunk listener immediately so
   * no further preview updates fire for a request the caller has moved on from.
   */
  cancelAssist(reqId: string): void {
    this.requests.unlisten(reqId);
    if (!this.transport || !this.hasAuthenticatedSession()) return;
    try {
      this.transport.send({ type: T.assistCancel, reqId, payload: null });
    } catch {
      // Best-effort — the transport may already be closing.
    }
  }

  /**
   * Cancel whatever `answerAssist` stream is currently pending, if any — an
   * explicit user "Cancel" click. The exact same retirement a NEW overlapping
   * `answerAssist` call already performs at its own start.
   */
  cancelCurrent(): void {
    this.supersedeAnyPendingAssist();
  }
}
