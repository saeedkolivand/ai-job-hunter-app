/**
 * Browser-extension ⇄ desktop protocol — zod-free constants & types.
 *
 * Split out from `extension-protocol.ts` so the browser extension can import the
 * wire-message constants and payload types WITHOUT pulling zod into its bundle
 * (zod v4's JIT `Function("")` feature-probe trips AMO's `DANGEROUS_EVAL` lint).
 * The zod schemas in `extension-protocol.ts` are bound to these interfaces so the
 * two can never drift. This module is pure data + types — no zod, no `window`,
 * no Node.
 */

/**
 * Every wire message `type`. v1 implements `import.request` / `import.result`;
 * `match.live` and `applied.check` are **reserved** — the type strings are
 * fixed now so a future build can add handlers without a protocol bump, but the
 * bridge does not handle them yet.
 */
export const EXTENSION_MESSAGE_TYPES = {
  /** Extension → desktop: import a job (URL mode, or Scan mode with `html`). */
  importRequest: 'import.request',
  /** Desktop → extension: the import outcome (or an `error`). */
  importResult: 'import.result',
  /** RESERVED — live ATS match for the open posting (not yet handled). */
  matchLive: 'match.live',
  /** RESERVED — "have I already applied to this URL?" (not yet handled). */
  appliedCheck: 'applied.check',
} as const;

/** Union of all wire `type` strings. */
export type ExtensionMessageType =
  (typeof EXTENSION_MESSAGE_TYPES)[keyof typeof EXTENSION_MESSAGE_TYPES];

/**
 * `import.request` payload. `html` present ⇒ Scan mode (the extension supplies
 * the authenticated DOM); absent ⇒ URL mode (the desktop fetches + scrapes).
 * `applied` flags the job as already applied (Saved origin otherwise → `saved`).
 */
export interface ExtensionImportRequest {
  url: string;
  html?: string;
  applied?: boolean;
}

/**
 * `import.result` payload. On success carries the created/merged
 * `applicationId` + its `status`, plus the parsed `title`/`company` so the
 * popup can confirm WHICH job was imported; `matchScore` is reserved for the
 * future live-match reply. On failure carries `error`.
 */
export interface ExtensionImportResult {
  applicationId?: string;
  status?: string;
  title?: string;
  company?: string;
  matchScore?: number;
  error?: string;
  /** True when the import was saved but the page could not be fully parsed (stub posting). */
  partial?: boolean;
}

/**
 * The transport envelope every frame is wrapped in. `payload` is left as
 * unknown here (each `type` narrows it via its own payload schema) so a single
 * envelope schema validates the frame shell before the handler dispatches.
 */
export interface ExtensionEnvelope {
  type: ExtensionMessageType;
  /** The paired secret. The bridge rejects any frame whose token mismatches. */
  token: string;
  /** Caller-chosen correlation id echoed back on the matching reply. */
  reqId: string;
  payload: unknown;
}
