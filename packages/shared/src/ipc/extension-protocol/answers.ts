import { z } from 'zod';

import {
  EXTENSION_ANSWER_ASSIST_MAX_CHARS,
  type ExtensionAnswerAssistRequest,
  type ExtensionAnswerAssistResult,
  type ExtensionAnswerPair,
  type ExtensionAnswersSaveRequest,
  type ExtensionAnswersSaveResult,
  type ExtensionAnswersSuggestRequest,
  type ExtensionAnswersSuggestResult,
  type ExtensionAnswerSuggestion,
  type ExtensionAssistChunkPayload,
} from '../extension-protocol-constants.js';

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
