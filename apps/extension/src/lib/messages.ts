/**
 * Internal popup ⇄ background message contract (NOT the wire protocol — that is
 * `@ajh/shared`'s extension-protocol). These travel over
 * `browser.runtime.sendMessage` inside the extension only.
 */

import type {
  ExtensionAnswersSaveResult,
  ExtensionAppliedCheckResult,
  ExtensionImportResult,
  ExtensionStatusUpdateResult,
} from '@ajh/shared';

import type { AutofillSummary } from './autofill';

/** Coarse connection state the popup renders. */
type ConnectionPhase =
  /** Background has not yet found a desktop bridge port. */
  | 'searching'
  /** A bridge port answered but no token is stored — show the pairing screen. */
  | 'not_paired'
  /** Paired + the v2 mutual handshake (incl. serverProof) with the desktop succeeded. */
  | 'connected'
  /** No bridge port answered in the probe range — the app is not running. */
  | 'app_not_running'
  /**
   * The desktop is too old to speak the v2 handshake (it never sent a
   * `challenge` — it closed the socket instead) — the user must UPDATE the
   * desktop app. Distinct from `bad_token` (a real token mismatch on a genuine
   * v2 desktop) and `app_not_running` (nothing answered at all).
   */
  | 'outdated'
  /** The stored token was rejected by the desktop — the user must re-pair. */
  | 'bad_token';

export interface ConnectionStatus {
  phase: ConnectionPhase;
  /** The bound port we connected to, when known (diagnostics only). */
  port: number | null;
  /** Whether a pairing token is currently stored. */
  hasToken: boolean;
}

/** popup → background requests. */
export type PopupRequest =
  | { kind: 'getStatus' }
  | { kind: 'setToken'; token: string }
  | { kind: 'clearToken' }
  | { kind: 'reconnect' }
  | { kind: 'import'; applied: boolean }
  /** Assisted autofill: fetch the profile fresh + inject the filler on this tab. */
  | { kind: 'fill' }
  /**
   * Fire-and-forget "have I already applied to this URL?" check for the
   * active tab, run once when the popup shows the connected view. Read-only;
   * never blocks the import controls.
   */
  | { kind: 'appliedCheck' }
  /**
   * User-clicked "Mark as applied" for the active tab's URL. Unlike
   * `appliedCheck`, this is a deliberate WRITE action — its failures are
   * surfaced to the user, never folded away.
   */
  | { kind: 'statusUpdate' }
  /**
   * User-clicked "Save my answers from this page": capture the active tab's
   * filled form fields and send them on as `answers.save`. Like
   * `statusUpdate`, this is a deliberate WRITE action — its failures are
   * surfaced to the user, never folded away.
   */
  | { kind: 'answersSave' };

/** background → popup responses (discriminated by the originating request). */
export type PopupResponse =
  | { ok: true; kind: 'status'; status: ConnectionStatus }
  | { ok: true; kind: 'token' }
  | { ok: true; kind: 'import'; result: ExtensionImportResult }
  | { ok: true; kind: 'fill'; summary: AutofillSummary }
  /**
   * Always `ok:true` — every failure mode (not paired, bridge down, malformed
   * reply, an old desktop's unrecognized message type) is folded into
   * `result.found === false` by the background, so the popup never has to
   * special-case an error path for this passive, best-effort check.
   */
  | { ok: true; kind: 'appliedCheck'; result: ExtensionAppliedCheckResult }
  /**
   * `ok:true` at the transport level; the desktop's own `ok`/`error` on
   * `result` is what the popup renders — this verb's failures are NOT
   * folded away (unlike `appliedCheck`), so the popup must check
   * `result.ok` itself.
   */
  | { ok: true; kind: 'statusUpdate'; result: ExtensionStatusUpdateResult }
  /**
   * `ok:true` at the transport level; like `statusUpdate`, the desktop's own
   * `ok`/`error` on `result` is what the popup renders — this verb's
   * failures are NOT folded away.
   */
  | { ok: true; kind: 'answersSave'; result: ExtensionAnswersSaveResult }
  | { ok: false; error: string };
