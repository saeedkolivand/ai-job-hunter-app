import { z } from 'zod';

import type {
  ExtensionAgentCallRequest,
  ExtensionAgentCallResult,
  ExtensionAgentQueryRequest,
  ExtensionAgentQueryResult,
  ExtensionMatchLiveRequest,
  ExtensionMatchLiveResult,
} from '../extension-protocol-constants.js';

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
