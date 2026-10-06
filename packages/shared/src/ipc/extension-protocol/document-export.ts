import { z } from 'zod';

import type {
  ExtensionDocumentExportRequest,
  ExtensionDocumentExportResult,
  ExtensionDocumentSource,
} from '../extension-protocol-constants.js';

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
