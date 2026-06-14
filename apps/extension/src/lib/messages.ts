/**
 * Internal popup ⇄ background message contract (NOT the wire protocol — that is
 * `@ajh/shared`'s extension-protocol). These travel over
 * `browser.runtime.sendMessage` inside the extension only.
 */

import type { ExtensionImportResult } from '@ajh/shared';

/** Coarse connection state the popup renders. */
type ConnectionPhase =
  /** Background has not yet found a desktop bridge port. */
  | 'searching'
  /** A bridge port answered but no token is stored — show the pairing screen. */
  | 'not_paired'
  /** Paired token present and an authenticated socket is open. */
  | 'connected'
  /** No bridge port answered in the probe range — the app is not running. */
  | 'app_not_running';

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
  | { kind: 'import'; mode: ImportMode; applied: boolean };

/** URL mode imports by tab URL only; Scan mode also ships the rendered DOM. */
export type ImportMode = 'url' | 'scan';

/** background → popup responses (discriminated by the originating request). */
export type PopupResponse =
  | { ok: true; kind: 'status'; status: ConnectionStatus }
  | { ok: true; kind: 'token' }
  | { ok: true; kind: 'import'; result: ExtensionImportResult }
  | { ok: false; error: string };
