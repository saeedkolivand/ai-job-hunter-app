/**
 * Browser-extension ⇄ desktop WebSocket bridge protocol (Feature 2).
 *
 * The single source of truth for the local WS frame format. The Rust side
 * (`apps/tauri/src-tauri/src/extension_bridge`) mirrors these string literals;
 * a parity test on the Rust side pins its message-type constants to the
 * `EXTENSION_MESSAGE_TYPES` values here so the two can never drift.
 *
 * Transport: a loopback-only (`127.0.0.1`) WebSocket. Every frame carries the
 * paired `token` secret and a `reqId` the caller correlates a reply to. This
 * module is pure data + Zod — no `window`, no Node.
 */

import { z } from 'zod';

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

export const ExtensionMessageTypeSchema = z.enum([
  EXTENSION_MESSAGE_TYPES.importRequest,
  EXTENSION_MESSAGE_TYPES.importResult,
  EXTENSION_MESSAGE_TYPES.matchLive,
  EXTENSION_MESSAGE_TYPES.appliedCheck,
]);

/**
 * `import.request` payload. `html` present ⇒ Scan mode (the extension supplies
 * the authenticated DOM); absent ⇒ URL mode (the desktop fetches + scrapes).
 * `applied` flags the job as already applied (Saved origin otherwise → `saved`).
 */
export const ExtensionImportRequestSchema = z.object({
  url: z.string().min(1),
  html: z.string().optional(),
  applied: z.boolean().optional(),
});
export type ExtensionImportRequest = z.infer<typeof ExtensionImportRequestSchema>;

/**
 * `import.result` payload. On success carries the created/merged
 * `applicationId` + its `status`; `matchScore` is reserved for the future
 * live-match reply. On failure carries `error`.
 */
export const ExtensionImportResultSchema = z.object({
  applicationId: z.string().optional(),
  status: z.string().optional(),
  matchScore: z.number().optional(),
  error: z.string().optional(),
});
export type ExtensionImportResult = z.infer<typeof ExtensionImportResultSchema>;

/**
 * The transport envelope every frame is wrapped in. `payload` is left as
 * unknown here (each `type` narrows it via its own payload schema) so a single
 * envelope schema validates the frame shell before the handler dispatches.
 */
export const ExtensionEnvelopeSchema = z.object({
  type: ExtensionMessageTypeSchema,
  /** The paired secret. The bridge rejects any frame whose token mismatches. */
  token: z.string().min(1),
  /** Caller-chosen correlation id echoed back on the matching reply. */
  reqId: z.string().min(1),
  payload: z.unknown(),
});
export type ExtensionEnvelope = z.infer<typeof ExtensionEnvelopeSchema>;
