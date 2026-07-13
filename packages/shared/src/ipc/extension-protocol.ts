/**
 * Browser-extension ⇄ desktop WebSocket bridge protocol (Feature 2).
 *
 * The single source of truth for the local WS frame format. The Rust side
 * (`apps/desktop/src-tauri/src/extension_bridge`) mirrors these string literals;
 * a parity test on the Rust side pins its message-type constants to the
 * `EXTENSION_MESSAGE_TYPES` values here so the two can never drift.
 *
 * The wire-message constants and payload TYPES live in the zod-free
 * `./extension-protocol-constants.ts` (so the browser extension can import them
 * without pulling zod into its bundle). This module owns the zod SCHEMAS and
 * binds each to its constants-file interface via `satisfies z.ZodType<…>` so the
 * schema and its type can never drift. It re-exports the constants/types so this
 * remains the single barrel entry for desktop/renderer consumers. Pure data +
 * Zod — no `window`, no Node.
 */

import { z } from 'zod';

import {
  EXTENSION_MESSAGE_TYPES,
  type ExtensionEnvelope,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
} from './extension-protocol-constants.js';

export {
  EXTENSION_MESSAGE_TYPES,
  type ExtensionEnvelope,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
};

export const ExtensionMessageTypeSchema = z.enum([
  EXTENSION_MESSAGE_TYPES.auth,
  EXTENSION_MESSAGE_TYPES.importRequest,
  EXTENSION_MESSAGE_TYPES.importResult,
  EXTENSION_MESSAGE_TYPES.profileGet,
  EXTENSION_MESSAGE_TYPES.profileResult,
  EXTENSION_MESSAGE_TYPES.matchLive,
  EXTENSION_MESSAGE_TYPES.appliedCheck,
]) satisfies z.ZodType<ExtensionMessageType>;

/**
 * `import.request` payload. `html` present ⇒ Scan mode (the extension supplies
 * the authenticated DOM); absent ⇒ URL mode (the desktop fetches + scrapes).
 * `applied` flags the job as already applied (Saved origin otherwise → `saved`).
 */
export const ExtensionImportRequestSchema = z.object({
  url: z.string().min(1),
  html: z.string().optional(),
  applied: z.boolean().optional(),
}) satisfies z.ZodType<ExtensionImportRequest>;

/**
 * `import.result` payload. On success carries the created/merged
 * `applicationId` + its `status`, plus the parsed `title`/`company` so the
 * popup can confirm WHICH job was imported; `matchScore` is reserved for the
 * future live-match reply. On failure carries `error`.
 */
export const ExtensionImportResultSchema = z.object({
  applicationId: z.string().optional(),
  status: z.string().optional(),
  title: z.string().optional(),
  company: z.string().optional(),
  matchScore: z.number().optional(),
  error: z.string().optional(),
  partial: z.boolean().optional(),
}) satisfies z.ZodType<ExtensionImportResult>;

/**
 * `profile.result` payload. Every profile field is optional (a sparse profile is
 * normal); `error` (present on refusal/failure) is mutually exclusive with the
 * fields in practice. Mirrors {@link ExtensionProfileResult}.
 */
export const ExtensionProfileResultSchema = z.object({
  fullName: z.string().optional(),
  email: z.string().optional(),
  phone: z.string().optional(),
  location: z.string().optional(),
  linkedin: z.string().optional(),
  github: z.string().optional(),
  website: z.string().optional(),
  error: z.string().optional(),
}) satisfies z.ZodType<ExtensionProfileResult>;

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
}) satisfies z.ZodType<ExtensionEnvelope>;
