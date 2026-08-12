/**
 * Pure field resolution for `ai_set_provider_settings`.
 *
 * The backend command is **REPLACE on every field**, not a patch: whatever the
 * call omits is written as NULL. A UI that saves one field therefore has to
 * send the other three too, or saving a base URL silently drops the model and
 * saving a model silently drops the context window (which is exactly the bug
 * this exists to prevent — the window is the only one with no other home, so
 * losing it makes a staged run fall back to the provider's default with no
 * visible change in Settings).
 *
 * Kept separate from the hook so the "what survives a save" rule is unit-
 * testable without React Query.
 *
 * If the command ever gains non-destructive absence (`field?: T | null`, where
 * absent = keep stored and `null` = clear), this function BODY is the only
 * thing that changes: callers already express intent as "what I am changing",
 * `null` already means "clear", and the hook would then send just those fields.
 * The `stored`/`localWindows` inputs become unnecessary, not wrong.
 */

import type { AiProviderRouting } from '@ajh/shared';

/** The `modelLimits` shape the renderer persists per local model. Only the
 *  window matters here; `maxTokens`/`temperature` never reach the backend. */
export type LocalModelWindows = Record<string, { contextWindow?: number } | undefined>;

export interface ProviderSettingsWriteInput {
  provider: string;
  /** The row as the backend holds it today (`activeConfig.providers[provider]`). */
  stored?: AiProviderRouting;
  /** The model to save. Absent = keep the stored one. */
  model?: string;
  /** `null` CLEARS the stored base URL; absent keeps it. (Only
   *  `openai-compatible` persists one — the backend nulls it for the rest.) */
  baseUrl?: string | null;
  /** An explicitly chosen window (the Settings slider). Wins over the stored
   *  per-model limit below. */
  contextWindow?: number;
  /** The renderer's `providers.ollama.modelLimits` — the only surface on which
   *  a user sets a window, keyed by model name. */
  localWindows?: LocalModelWindows;
}

/** Exactly the four fields `setProviderSettings` replaces. */
export interface ProviderSettingsWrite {
  provider: string;
  model?: string;
  baseUrl?: string;
  contextWindow?: number;
}

/**
 * Build the full REPLACE payload for one provider row.
 *
 * The window **belongs to the model**, so it is resolved from the model being
 * saved rather than carried along blindly: a stored window survives a save that
 * doesn't touch the model, and is dropped (never re-attached to a different
 * model) when the model changes and the new one has no window of its own.
 */
export function resolveProviderSettingsWrite(
  input: ProviderSettingsWriteInput
): ProviderSettingsWrite {
  const { provider, stored, localWindows } = input;

  const model = input.model ?? stored?.model;
  const baseUrl = input.baseUrl === null ? undefined : (input.baseUrl ?? stored?.baseUrl);

  const modelChanged = model !== stored?.model;
  const storedWindow = modelChanged ? undefined : stored?.contextWindow;
  const modelWindow = model ? localWindows?.[model]?.contextWindow : undefined;
  const contextWindow = input.contextWindow ?? modelWindow ?? storedWindow;

  return { provider, model, baseUrl, contextWindow };
}
