import { z } from 'zod';

import type {
  ExtensionSettingsGetRequest,
  ExtensionSettingsKey,
  ExtensionSettingsResult,
  ExtensionSettingsSetRequest,
  ExtensionSettingsValues,
} from '../extension-protocol-constants.js';

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

/** The live values of every switch. Mirrors {@link ExtensionSettingsValues}.
 *  `saveAnswersOnSubmit` (the fourth key, PR4) is optional ON THE WIRE ONLY —
 *  a protocol-v2 desktop from before this PR answers `settings.result` with
 *  just the first three keys, so a MISSING fourth key is normalized to
 *  `false` (the safe "off" default) rather than failing the whole parse; a
 *  PRESENT but wrong-typed value still fails. The parsed/output shape is
 *  still every field required, matching {@link ExtensionSettingsValues}. */
export const ExtensionSettingsValuesSchema = z.object({
  autofill: z.boolean(),
  aiAssist: z.boolean(),
  autotrack: z.boolean(),
  saveAnswersOnSubmit: z.boolean().optional().default(false),
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
