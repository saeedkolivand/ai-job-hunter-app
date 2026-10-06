import { z } from 'zod';

import {
  EXTENSION_MESSAGE_TYPES,
  type ExtensionAuthOkPayload,
  type ExtensionAuthPayload,
  type ExtensionChallengePayload,
  type ExtensionEnvelope,
  type ExtensionHelloPayload,
  type ExtensionMessageType,
} from '../extension-protocol-constants.js';

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
