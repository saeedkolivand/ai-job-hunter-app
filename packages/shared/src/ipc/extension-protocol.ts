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
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_ANSWER_ASSIST_MAX_CHARS,
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_NO_PROVIDER_MESSAGE,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionAgentCallRequest,
  type ExtensionAgentCallResult,
  type ExtensionAgentQueryRequest,
  type ExtensionAgentQueryResult,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerAssistTopic,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveRequest,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestRequest,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAppliedBatchEntry,
  type ExtensionAppliedCheckBatchRequest,
  type ExtensionAppliedCheckBatchResult,
  type ExtensionAppliedCheckRequest,
  type ExtensionAppliedCheckResult,
  type ExtensionAssistChunkPayload,
  type ExtensionAuthOkPayload,
  type ExtensionAuthPayload,
  type ExtensionAutofillResult,
  type ExtensionAutotrackResult,
  type ExtensionChallengePayload,
  type ExtensionDocumentExportRequest,
  type ExtensionDocumentExportResult,
  type ExtensionDocumentSource,
  type ExtensionEnvelope,
  type ExtensionHelloPayload,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveRequest,
  type ExtensionMatchLiveResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
  type ExtensionRewritePreset,
  type ExtensionSettingsGetRequest,
  type ExtensionSettingsKey,
  type ExtensionSettingsResult,
  type ExtensionSettingsSetRequest,
  type ExtensionSettingsValues,
  type ExtensionStatusUpdateRequest,
  type ExtensionStatusUpdateResult,
  HANDSHAKE_DOMAIN,
  HANDSHAKE_TEST_VECTOR,
  handshakeMessage,
  type HandshakeRole,
  MAX_APPLIED_CHECK_BATCH_URLS,
} from './extension-protocol-constants.js';

export {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_ANSWER_ASSIST_MAX_CHARS,
  EXTENSION_MESSAGE_TYPES,
  EXTENSION_NO_PROVIDER_MESSAGE,
  EXTENSION_PROTOCOL_VERSION,
  type ExtensionAgentCallRequest,
  type ExtensionAgentCallResult,
  type ExtensionAgentQueryRequest,
  type ExtensionAgentQueryResult,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerAssistTopic,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveRequest,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestRequest,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAppliedBatchEntry,
  type ExtensionAppliedCheckBatchRequest,
  type ExtensionAppliedCheckBatchResult,
  type ExtensionAppliedCheckRequest,
  type ExtensionAppliedCheckResult,
  type ExtensionAssistChunkPayload,
  type ExtensionAuthOkPayload,
  type ExtensionAuthPayload,
  type ExtensionAutofillResult,
  type ExtensionAutotrackResult,
  type ExtensionChallengePayload,
  type ExtensionDocumentExportRequest,
  type ExtensionDocumentExportResult,
  type ExtensionDocumentSource,
  type ExtensionEnvelope,
  type ExtensionHelloPayload,
  type ExtensionImportRequest,
  type ExtensionImportResult,
  type ExtensionMatchLiveRequest,
  type ExtensionMatchLiveResult,
  type ExtensionMessageType,
  type ExtensionProfileResult,
  type ExtensionRewritePreset,
  type ExtensionSettingsGetRequest,
  type ExtensionSettingsKey,
  type ExtensionSettingsResult,
  type ExtensionSettingsSetRequest,
  type ExtensionSettingsValues,
  type ExtensionStatusUpdateRequest,
  type ExtensionStatusUpdateResult,
  HANDSHAKE_DOMAIN,
  HANDSHAKE_TEST_VECTOR,
  handshakeMessage,
  type HandshakeRole,
  MAX_APPLIED_CHECK_BATCH_URLS,
};

export const ExtensionMessageTypeSchema = z.enum([
  EXTENSION_MESSAGE_TYPES.hello,
  EXTENSION_MESSAGE_TYPES.challenge,
  EXTENSION_MESSAGE_TYPES.auth,
  EXTENSION_MESSAGE_TYPES.authOk,
  EXTENSION_MESSAGE_TYPES.updateRequired,
  EXTENSION_MESSAGE_TYPES.tokenRevoked,
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
  EXTENSION_MESSAGE_TYPES.autotrackCheck,
  EXTENSION_MESSAGE_TYPES.autotrackResult,
  EXTENSION_MESSAGE_TYPES.autofillCheck,
  EXTENSION_MESSAGE_TYPES.autofillResult,
  EXTENSION_MESSAGE_TYPES.answersSave,
  EXTENSION_MESSAGE_TYPES.answersResult,
  EXTENSION_MESSAGE_TYPES.answersSuggest,
  EXTENSION_MESSAGE_TYPES.answersSuggestResult,
  EXTENSION_MESSAGE_TYPES.answerAssist,
  EXTENSION_MESSAGE_TYPES.answerAssistResult,
  EXTENSION_MESSAGE_TYPES.assistChunk,
  EXTENSION_MESSAGE_TYPES.assistDone,
  EXTENSION_MESSAGE_TYPES.assistCancel,
  EXTENSION_MESSAGE_TYPES.agentQuery,
  EXTENSION_MESSAGE_TYPES.agentResult,
  EXTENSION_MESSAGE_TYPES.agentCall,
  EXTENSION_MESSAGE_TYPES.agentCallResult,
  EXTENSION_MESSAGE_TYPES.settingsGet,
  EXTENSION_MESSAGE_TYPES.settingsResult,
  EXTENSION_MESSAGE_TYPES.settingsSet,
  EXTENSION_MESSAGE_TYPES.documentExport,
  EXTENSION_MESSAGE_TYPES.documentResult,
  EXTENSION_MESSAGE_TYPES.appliedCheckBatch,
  EXTENSION_MESSAGE_TYPES.appliedBatchResult,
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
  // Additive/optional (PR4) — marks an AUTOMATED save from the submit-watcher's
  // OWN save-answers-on-submit opt-in. The desktop honors it only when that
  // opt-in is on; absent/false is the ordinary user-clicked save.
  auto: z.boolean().optional(),
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
 * `answer.assist` payload — the question to draft an answer for (`mode`
 * omitted/`'draft'`), or (PR 11) an existing answer to rewrite (`mode:
 * 'rewrite'` — `existingAnswer`/`preset`/`instruction`). Mirrors
 * {@link ExtensionAnswerAssistRequest}. Shape-only (no byte caps): the
 * desktop clamps every field at the resolve boundary, matching the sibling
 * request schemas above — `maxChars` included: the wire pins its SHAPE
 * (a positive integer) and nothing else, deliberately NOT
 * {@link EXTENSION_ANSWER_ASSIST_MAX_CHARS}. That constant is the desktop's
 * clamp, not a wire bound; enforcing it here would turn an over-large limit
 * from a value the desktop quietly reduces into a rejected request, which
 * is how a client on a different version gets a legitimate draft refused
 * over a number. The desktop re-validates and clamps it either way
 * (`parse_max_chars`), treating it as untrusted like every other field.
 */
export const ExtensionAnswerAssistRequestSchema = z.object({
  question: z.string().min(1),
  url: z.string().optional(),
  searchWeb: z.boolean().optional(),
  mode: z.enum(['draft', 'rewrite']).optional(),
  existingAnswer: z.string().optional(),
  preset: z.enum(['shorten', 'expand', 'rephrase', 'impact', 'grammar']).optional(),
  instruction: z.string().optional(),
  maxChars: z.number().int().positive().optional(),
  // Additive/optional (PR4) — see ExtensionAnswerAssistTopic's doc.
  topic: z.enum(['company-brief', 'salary-answer']).optional(),
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
    // PR3 — two verbatim salary facts, never a verdict. Additive/optional:
    // an older desktop never sends it, an older extension ignores it.
    salary: z.object({ posting: z.string(), expectation: z.string().optional() }).optional(),
  }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionMatchLiveResult>;

/**
 * `agent.query` payload for the extension read tier (PR1, ADR-050). Mirrors
 * {@link ExtensionAgentQueryRequest} — `resource` required, every other
 * string-keyed field passed through as-is (`.catchall`, the resource-specific
 * parameters spread at the payload's top level; no per-resource validation
 * here — the desktop's own `agent_read` resolver validates those against the
 * resource it names).
 */
export const ExtensionAgentQueryRequestSchema = z
  .object({ resource: z.string().min(1) })
  .catchall(z.unknown()) satisfies z.ZodType<ExtensionAgentQueryRequest>;

/**
 * `agent.result` payload. Mirrors {@link ExtensionAgentQueryResult} — a
 * discriminated union on `ok`: `ok:true` requires the echoed `resource` +
 * an opaque `data`; `ok:false` requires the echoed `resource` + a
 * user-facing `error` (`detail`/`retryAfterMs` optional — the latter set
 * only on a throttle refusal).
 */
export const ExtensionAgentQueryResultSchema = z.discriminatedUnion('ok', [
  z.object({ ok: z.literal(true), resource: z.string(), data: z.unknown() }),
  z.object({
    ok: z.literal(false),
    resource: z.string(),
    error: z.string(),
    detail: z.string().optional(),
    retryAfterMs: z.number().optional(),
  }),
]) satisfies z.ZodType<ExtensionAgentQueryResult>;

/**
 * `agent.call` payload for the extension read tier (PR1, ADR-050). Mirrors
 * {@link ExtensionAgentCallRequest} — shape-only: a flat `namespace`/
 * `command` pair, re-validated desktop-side against the policy table (only
 * `Effect::Read` rows ever dispatch for this caller).
 */
export const ExtensionAgentCallRequestSchema = z.object({
  namespace: z.string().min(1),
  command: z.string().min(1),
  input: z.unknown().optional(),
}) satisfies z.ZodType<ExtensionAgentCallRequest>;

/**
 * `agent.call.result` payload. Mirrors {@link ExtensionAgentCallResult} — a
 * discriminated union on `dispatched` (never `ok`, ADR-038 §5):
 * `dispatched:true` requires `namespace`/`command` + an opaque `data`;
 * `dispatched:false` requires `namespace`/`command` + a user-facing `error`
 * (`detail`/`retryAfterMs` optional — the latter set only on a throttle
 * refusal).
 */
export const ExtensionAgentCallResultSchema = z.discriminatedUnion('dispatched', [
  z.object({
    dispatched: z.literal(true),
    namespace: z.string(),
    command: z.string(),
    data: z.unknown(),
  }),
  z.object({
    dispatched: z.literal(false),
    namespace: z.string(),
    command: z.string(),
    error: z.string(),
    detail: z.string().optional(),
    retryAfterMs: z.number().optional(),
  }),
]) satisfies z.ZodType<ExtensionAgentCallResult>;

/** The extension's opt-in switch keys. Mirrors {@link ExtensionSettingsKey}. */
export const ExtensionSettingsKeySchema = z.enum([
  'autofill',
  'aiAssist',
  'autotrack',
  'saveAnswersOnSubmit',
]) satisfies z.ZodType<ExtensionSettingsKey>;

/** `settings.get` payload — no fields, and none allowed (`.strict()` rejects a
 *  surplus key rather than silently ignoring it). Mirrors
 *  {@link ExtensionSettingsGetRequest}. */
export const ExtensionSettingsGetRequestSchema = z.strictObject(
  {}
) satisfies z.ZodType<ExtensionSettingsGetRequest>;

/** `settings.set` payload — flip exactly one switch. Mirrors {@link ExtensionSettingsSetRequest}. */
export const ExtensionSettingsSetRequestSchema = z.object({
  key: ExtensionSettingsKeySchema,
  enabled: z.boolean(),
}) satisfies z.ZodType<ExtensionSettingsSetRequest>;

/** The live values of every switch. Mirrors {@link ExtensionSettingsValues}. */
export const ExtensionSettingsValuesSchema = z.object({
  autofill: z.boolean(),
  aiAssist: z.boolean(),
  autotrack: z.boolean(),
  saveAnswersOnSubmit: z.boolean(),
}) satisfies z.ZodType<ExtensionSettingsValues>;

/**
 * `settings.result` payload — answers BOTH `settings.get` and `settings.set`.
 * Mirrors {@link ExtensionSettingsResult} — a discriminated union on `ok`:
 * `ok:true` requires the full `settings` object; `ok:false` requires a
 * user-facing `error`.
 */
export const ExtensionSettingsResultSchema = z.discriminatedUnion('ok', [
  z.object({ ok: z.literal(true), settings: ExtensionSettingsValuesSchema }),
  z.object({ ok: z.literal(false), error: z.string() }),
]) satisfies z.ZodType<ExtensionSettingsResult>;

/** `document.export`'s `source` field — a per-job generation or a saved base
 *  document. Mirrors {@link ExtensionDocumentSource}. */
export const ExtensionDocumentSourceSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('generation'), url: z.string().min(1) }),
  z.object({ kind: z.literal('document'), id: z.string().min(1) }),
]) satisfies z.ZodType<ExtensionDocumentSource>;

/**
 * `document.export` payload (PR2). Shape-only, like every sibling request
 * schema above — `templateId`/`letterLayoutId` are free strings here; the
 * desktop's own `ExportRequest` serde is what falls back on an unknown id.
 * Mirrors {@link ExtensionDocumentExportRequest}.
 */
export const ExtensionDocumentExportRequestSchema = z.object({
  source: ExtensionDocumentSourceSchema,
  kind: z.enum(['resume', 'cover-letter']),
  format: z.enum(['pdf', 'docx', 'txt']),
  templateId: z.string().min(1),
  letterLayoutId: z.string().optional(),
  atsMode: z.boolean().optional(),
}) satisfies z.ZodType<ExtensionDocumentExportRequest>;

/**
 * `document.result` payload. Mirrors {@link ExtensionDocumentExportResult} —
 * a discriminated union on `ok`: `ok:true` requires the base64 `data` + the
 * literal `dataEncoding: 'base64'` + `mimeType`/`filename`/`byteLength` +
 * the echoed `kind`/`format`/`templateId`; `ok:false` requires a
 * user-facing `error` (`detail`/`retryAfterMs` optional — the latter set
 * only on a throttle refusal, same shape as {@link ExtensionAgentQueryResultSchema}).
 */
export const ExtensionDocumentExportResultSchema = z.discriminatedUnion('ok', [
  z.object({
    ok: z.literal(true),
    data: z.string(),
    dataEncoding: z.literal('base64'),
    mimeType: z.string(),
    filename: z.string(),
    byteLength: z.number(),
    kind: z.enum(['resume', 'cover-letter']),
    format: z.enum(['pdf', 'docx', 'txt']),
    templateId: z.string(),
  }),
  z.object({
    ok: z.literal(false),
    error: z.string(),
    detail: z.string().optional(),
    retryAfterMs: z.number().optional(),
  }),
]) satisfies z.ZodType<ExtensionDocumentExportResult>;

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
