import { z } from 'zod';

import {
  type ExtensionAppliedBatchEntry,
  type ExtensionAppliedCheckBatchRequest,
  type ExtensionAppliedCheckBatchResult,
  type ExtensionAppliedCheckRequest,
  type ExtensionAppliedCheckResult,
  type ExtensionAutofillResult,
  type ExtensionAutotrackResult,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveResult,
  type ExtensionProfileResult,
  type ExtensionStatusUpdateRequest,
  type ExtensionStatusUpdateResult,
  MAX_APPLIED_CHECK_BATCH_URLS,
} from '../extension-protocol-constants.js';

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
 * popup can confirm WHICH job was imported. `matchScore` is a best-effort
 * keyword-only ATS score (0–100) against the user's default/most-recent
 * résumé — see {@link ExtensionMatchLiveResult}'s doc for why it is always
 * keyword-only; it is OMITTED (not `0`/`null`) whenever scoring failed for any
 * reason (no résumé saved yet, unusable posting text, a scoring timeout) — the
 * import itself always succeeds regardless of whether this field is present.
 * On failure carries `error`.
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
 * fields in practice. `extraLinks` is additive/optional — absent on an old
 * desktop's reply, ignored by an old extension — and each entry is validated as
 * a plain `{label, url}` shape here (the non-empty-label / http(s)-url / cap-of-10
 * rules are enforced desktop-side before the payload is ever sent). Mirrors
 * {@link ExtensionProfileResult}.
 */
export const ExtensionProfileResultSchema = z.object({
  fullName: z.string().optional(),
  email: z.string().optional(),
  phone: z.string().optional(),
  location: z.string().optional(),
  linkedin: z.string().optional(),
  github: z.string().optional(),
  website: z.string().optional(),
  extraLinks: z.array(z.object({ label: z.string(), url: z.string() })).optional(),
  error: z.string().optional(),
}) satisfies z.ZodType<ExtensionProfileResult>;

/**
 * `applied.check` payload — the active tab's URL to look up. Mirrors
 * {@link ExtensionAppliedCheckRequest}.
 */
export const ExtensionAppliedCheckRequestSchema = z.object({
  url: z.string().min(1),
}) satisfies z.ZodType<ExtensionAppliedCheckRequest>;

/**
 * `applied.result` payload. `found` is required; every other field is
 * optional (populated only when an Application was found, or `error` on a
 * malformed/empty url). Mirrors {@link ExtensionAppliedCheckResult}.
 */
export const ExtensionAppliedCheckResultSchema = z.object({
  found: z.boolean(),
  applicationId: z.string().optional(),
  status: z.string().optional(),
  title: z.string().optional(),
  appliedAt: z.number().optional(),
  error: z.string().optional(),
}) satisfies z.ZodType<ExtensionAppliedCheckResult>;

/**
 * `applied.check.batch` payload (PR3, results-page stamps). Enforces the same
 * {@link MAX_APPLIED_CHECK_BATCH_URLS} cap the desktop's `parse_urls` refuses
 * over (`too_many_urls`, never truncated) — unlike every sibling request
 * schema above, an unbounded `urls` array here would let a caller pass shared
 * validation with a request the Rust IPC handler is guaranteed to refuse.
 * Mirrors {@link ExtensionAppliedCheckBatchRequest}.
 */
export const ExtensionAppliedCheckBatchRequestSchema = z.object({
  urls: z.array(z.string()).max(MAX_APPLIED_CHECK_BATCH_URLS),
}) satisfies z.ZodType<ExtensionAppliedCheckBatchRequest>;

/** One url's outcome in an `applied.batch.result` reply. Mirrors
 *  {@link ExtensionAppliedBatchEntry}. */
export const ExtensionAppliedBatchEntrySchema = z.object({
  url: z.string(),
  found: z.boolean(),
  status: z.string().optional(),
}) satisfies z.ZodType<ExtensionAppliedBatchEntry>;

/**
 * `applied.check.batch` payload. Mirrors {@link ExtensionAppliedCheckBatchResult}
 * — a discriminated union on `ok`: `ok:true` requires a `results` array;
 * `ok:false` requires `error` (`detail`/`retryAfterMs` optional — the latter
 * set only on a throttle refusal, same shape as
 * {@link ExtensionDocumentExportResultSchema}).
 */
export const ExtensionAppliedCheckBatchResultSchema = z.discriminatedUnion('ok', [
  z.object({ ok: z.literal(true), results: z.array(ExtensionAppliedBatchEntrySchema) }),
  z.object({
    ok: z.literal(false),
    error: z.string(),
    detail: z.string().optional(),
    retryAfterMs: z.number().optional(),
  }),
]) satisfies z.ZodType<ExtensionAppliedCheckBatchResult>;

/**
 * `status.update` payload — the url to mark applied. `to` is a literal, not a
 * free string: the allowlist is visible in the contract itself, not just the
 * Rust re-validation. Mirrors {@link ExtensionStatusUpdateRequest}.
 */
export const ExtensionStatusUpdateRequestSchema = z.object({
  url: z.string().min(1),
  to: z.literal('applied'),
  // Additive/optional: marks an AUTOMATED write from the gesture submit-watcher
  // (Task #22). The desktop honors it only when the auto-track opt-in is on;
  // absent/false is the ordinary user-clicked "Mark as applied".
  auto: z.boolean().optional(),
}) satisfies z.ZodType<ExtensionStatusUpdateRequest>;

/**
 * `autotrack.result` payload — the desktop-enforced auto-track opt-in state
 * (Task #22). Mirrors {@link ExtensionAutotrackResult}. Read by the extension
 * before arming the gesture submit-watcher; a malformed reply is treated as
 * `false` (OFF) client-side.
 */
export const ExtensionAutotrackResultSchema = z.object({
  enabled: z.boolean(),
}) satisfies z.ZodType<ExtensionAutotrackResult>;

/**
 * `autofill.result` payload — the desktop-enforced assisted-autofill opt-in
 * state (Task #30). Mirrors {@link ExtensionAutofillResult} exactly (same
 * shape as {@link ExtensionAutotrackResultSchema}). A malformed reply is
 * treated as `false` (OFF) client-side.
 */
export const ExtensionAutofillResultSchema = z.object({
  enabled: z.boolean(),
}) satisfies z.ZodType<ExtensionAutofillResult>;

/**
 * `status.update` payload. Mirrors {@link ExtensionStatusUpdateResult} — a
 * discriminated union on `ok` so success/failure fields can never mix:
 * `ok:true` requires `applicationId` + the literal `status: 'applied'`;
 * `ok:false` requires `error`.
 */
export const ExtensionStatusUpdateResultSchema = z.discriminatedUnion('ok', [
  z.object({ ok: z.literal(true), applicationId: z.string(), status: z.literal('applied') }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionStatusUpdateResult>;
