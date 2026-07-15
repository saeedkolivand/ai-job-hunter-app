/**
 * Browser-extension ⇄ desktop WebSocket bridge protocol (Feature 2).
 *
 * The single source of truth for the local WS frame format. The Rust side
 * (`apps/desktop/src-tauri/src/extension_bridge`) mirrors these string literals;
 * a parity test on the Rust side pins its message-type constants to the
 * `EXTENSION_MESSAGE_TYPES` values here so the two can never drift.
 *
 * The wire-message constants, the handshake message canonicalization, and the
 * payload TYPES live in the zod-free `./extension-protocol-constants.ts` (so the
 * browser extension can import them without pulling zod into its bundle). This
 * module owns the zod SCHEMAS and binds each to its constants-file interface via
 * `satisfies z.ZodType<…>` so the schema and its type can never drift. It
 * re-exports the constants/types so this remains the single barrel entry for
 * desktop/renderer consumers. Pure data + Zod — no `window`, no Node.
 */

import { z } from 'zod';

import {
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveRequest,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestRequest,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAppliedCheckRequest,
  type ExtensionAppliedCheckResult,
  type ExtensionAssistChunkPayload,
  type ExtensionAuthOkPayload,
  type ExtensionAuthPayload,
  type ExtensionChallengePayload,
  type ExtensionEnvelope,
  type ExtensionHelloPayload,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveRequest,
  type ExtensionMatchLiveResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
  type ExtensionStatusUpdateRequest,
  type ExtensionStatusUpdateResult,
  HANDSHAKE_DOMAIN,
  HANDSHAKE_TEST_VECTOR,
  handshakeMessage,
  type HandshakeRole,
} from './extension-protocol-constants.js';

export {
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveRequest,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestRequest,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAppliedCheckRequest,
  type ExtensionAppliedCheckResult,
  type ExtensionAssistChunkPayload,
  type ExtensionAuthOkPayload,
  type ExtensionAuthPayload,
  type ExtensionChallengePayload,
  type ExtensionEnvelope,
  type ExtensionHelloPayload,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveRequest,
  type ExtensionMatchLiveResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
  type ExtensionStatusUpdateRequest,
  type ExtensionStatusUpdateResult,
  HANDSHAKE_DOMAIN,
  HANDSHAKE_TEST_VECTOR,
  handshakeMessage,
  type HandshakeRole,
};

export const ExtensionMessageTypeSchema = z.enum([
  EXTENSION_MESSAGE_TYPES.hello,
  EXTENSION_MESSAGE_TYPES.challenge,
  EXTENSION_MESSAGE_TYPES.auth,
  EXTENSION_MESSAGE_TYPES.authOk,
  EXTENSION_MESSAGE_TYPES.updateRequired,
  EXTENSION_MESSAGE_TYPES.importRequest,
  EXTENSION_MESSAGE_TYPES.importResult,
  EXTENSION_MESSAGE_TYPES.profileGet,
  EXTENSION_MESSAGE_TYPES.profileResult,
  EXTENSION_MESSAGE_TYPES.matchLive,
  EXTENSION_MESSAGE_TYPES.matchResult,
  EXTENSION_MESSAGE_TYPES.appliedCheck,
  EXTENSION_MESSAGE_TYPES.appliedResult,
  EXTENSION_MESSAGE_TYPES.statusUpdate,
  EXTENSION_MESSAGE_TYPES.statusResult,
  EXTENSION_MESSAGE_TYPES.answersSave,
  EXTENSION_MESSAGE_TYPES.answersResult,
  EXTENSION_MESSAGE_TYPES.answersSuggest,
  EXTENSION_MESSAGE_TYPES.answersSuggestResult,
  EXTENSION_MESSAGE_TYPES.answerAssist,
  EXTENSION_MESSAGE_TYPES.answerAssistResult,
  EXTENSION_MESSAGE_TYPES.assistChunk,
  EXTENSION_MESSAGE_TYPES.assistDone,
  EXTENSION_MESSAGE_TYPES.assistCancel,
]) satisfies z.ZodType<ExtensionMessageType>;

/** `hello` payload (handshake step 1). No token — the proof authenticates later. */
export const ExtensionHelloPayloadSchema = z.object({
  protocol: z.number().int().positive(),
  clientNonce: z.string().min(1),
}) satisfies z.ZodType<ExtensionHelloPayload>;

/** `challenge` payload (handshake step 2). */
export const ExtensionChallengePayloadSchema = z.object({
  serverNonce: z.string().min(1),
}) satisfies z.ZodType<ExtensionChallengePayload>;

/** `auth` payload (handshake step 3) — the client proof, NOT the token. */
export const ExtensionAuthPayloadSchema = z.object({
  proof: z.string().min(1),
}) satisfies z.ZodType<ExtensionAuthPayload>;

/** `auth.ok` payload (handshake step 4) — the server proof the extension verifies. */
export const ExtensionAuthOkPayloadSchema = z.object({
  serverProof: z.string().min(1),
}) satisfies z.ZodType<ExtensionAuthOkPayload>;

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
 * `status.update` payload — the url to mark applied. `to` is a literal, not a
 * free string: the allowlist is visible in the contract itself, not just the
 * Rust re-validation. Mirrors {@link ExtensionStatusUpdateRequest}.
 */
export const ExtensionStatusUpdateRequestSchema = z.object({
  url: z.string().min(1),
  to: z.literal('applied'),
}) satisfies z.ZodType<ExtensionStatusUpdateRequest>;

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

/** One captured `{question, answer}` pair. Mirrors {@link ExtensionAnswerPair}. */
export const ExtensionAnswerPairSchema = z.object({
  question: z.string(),
  answer: z.string(),
}) satisfies z.ZodType<ExtensionAnswerPair>;

/**
 * `answers.save` payload — the url plus the captured pairs to append. Mirrors
 * {@link ExtensionAnswersSaveRequest}. Shape-only (no byte/entry caps): the
 * desktop store boundary is the real clamp, matching the sibling request
 * schemas above (`ExtensionImportRequestSchema` et al. don't cap either).
 */
export const ExtensionAnswersSaveRequestSchema = z.object({
  url: z.string().min(1),
  answers: z.array(ExtensionAnswerPairSchema),
}) satisfies z.ZodType<ExtensionAnswersSaveRequest>;

/**
 * `answers.save` payload. Mirrors {@link ExtensionAnswersSaveResult} — a
 * discriminated union on `ok`: `ok:true` requires `applicationId` + numeric
 * `saved`/`skipped` (title/company optional); `ok:false` requires `error`.
 */
export const ExtensionAnswersSaveResultSchema = z.discriminatedUnion('ok', [
  z.object({
    ok: z.literal(true),
    applicationId: z.string(),
    saved: z.number(),
    skipped: z.number(),
    title: z.string().optional(),
    company: z.string().optional(),
  }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionAnswersSaveResult>;

/**
 * `answers.suggest` payload — the (client-capped) labels to fuzzy-match.
 * Mirrors {@link ExtensionAnswersSuggestRequest}. Shape-only (no byte/entry
 * caps): the desktop matcher boundary is the real clamp, matching the sibling
 * request schemas above.
 */
export const ExtensionAnswersSuggestRequestSchema = z.object({
  questions: z.array(z.string()),
}) satisfies z.ZodType<ExtensionAnswersSuggestRequest>;

/** One matched suggestion. Mirrors {@link ExtensionAnswerSuggestion}. */
export const ExtensionAnswerSuggestionSchema = z.object({
  question: z.string(),
  answer: z.string(),
  sourceCompany: z.string().optional(),
  sourceTitle: z.string().optional(),
  sourceQuestion: z.string(),
  score: z.number(),
  salary: z.boolean(),
}) satisfies z.ZodType<ExtensionAnswerSuggestion>;

/**
 * `answers.suggest` payload. Mirrors {@link ExtensionAnswersSuggestResult} —
 * a discriminated union on `ok`: `ok:true` requires a `suggestions` array;
 * `ok:false` requires `error`.
 */
export const ExtensionAnswersSuggestResultSchema = z.discriminatedUnion('ok', [
  z.object({ ok: z.literal(true), suggestions: z.array(ExtensionAnswerSuggestionSchema) }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionAnswersSuggestResult>;

/**
 * `answer.assist` payload — the question to draft an answer for. Mirrors
 * {@link ExtensionAnswerAssistRequest}. Shape-only (no byte cap): the desktop
 * clamps the question at the resolve boundary, matching the sibling request
 * schemas above.
 */
export const ExtensionAnswerAssistRequestSchema = z.object({
  question: z.string().min(1),
  url: z.string().optional(),
  searchWeb: z.boolean().optional(),
}) satisfies z.ZodType<ExtensionAnswerAssistRequest>;

/**
 * `answer.assist` payload. Mirrors {@link ExtensionAnswerAssistResult} — a
 * discriminated union on `ok`: `ok:true` requires the echoed `question` +
 * the finished `draft` + a `sourced` flags object; `ok:false` requires
 * `error`.
 */
export const ExtensionAnswerAssistResultSchema = z.discriminatedUnion('ok', [
  z.object({
    ok: z.literal(true),
    question: z.string(),
    draft: z.string(),
    sourced: z.object({
      web: z.boolean().optional(),
      brief: z.boolean().optional(),
      salary: z.boolean().optional(),
    }),
  }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionAnswerAssistResult>;

/**
 * `assist.chunk` payload — one incremental delta of a streaming reply.
 * Mirrors {@link ExtensionAssistChunkPayload}. `assist.done`/`assist.cancel`
 * carry no payload (the envelope's own `reqId` is the whole message), so
 * they have no dedicated schema — `ExtensionEnvelopeSchema`'s `payload:
 * z.unknown()` already accepts anything, including `null`.
 */
export const ExtensionAssistChunkPayloadSchema = z.object({
  delta: z.string(),
}) satisfies z.ZodType<ExtensionAssistChunkPayload>;

/**
 * `match.live` payload — the Scan-mode capture to score. Mirrors
 * {@link ExtensionMatchLiveRequest}. `html` is required (not optional, unlike
 * `import.request`'s) — there is no URL-mode fallback for this verb.
 */
export const ExtensionMatchLiveRequestSchema = z.object({
  url: z.string().min(1),
  html: z.string().min(1),
}) satisfies z.ZodType<ExtensionMatchLiveRequest>;

/**
 * `match.live` payload. Mirrors {@link ExtensionMatchLiveResult} — a
 * discriminated union on `ok`: `ok:true` requires `combined`/`ats`/`gaps`/
 * `resumeName`/`scoreSource` (the optional `semantic` is wire-reserved, never
 * populated by the current desktop implementation — see that type's doc);
 * `ok:false` requires `error`.
 */
export const ExtensionMatchLiveResultSchema = z.discriminatedUnion('ok', [
  z.object({
    ok: z.literal(true),
    combined: z.number(),
    ats: z.number(),
    semantic: z.number().optional(),
    gaps: z.array(z.string()),
    resumeName: z.string(),
    scoreSource: z.enum(['keyword', 'combined']),
  }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionMatchLiveResult>;

/**
 * The transport envelope every frame is wrapped in. `payload` is left as
 * unknown here (each `type` narrows it via its own payload schema) so a single
 * envelope schema validates the frame shell before the handler dispatches.
 *
 * v2 removed the `token` field entirely: the handshake authenticates the socket,
 * so no frame carries the pairing secret.
 */
export const ExtensionEnvelopeSchema = z.object({
  type: ExtensionMessageTypeSchema,
  /** Caller-chosen correlation id echoed back on the matching reply. */
  reqId: z.string().min(1),
  payload: z.unknown(),
}) satisfies z.ZodType<ExtensionEnvelope>;
