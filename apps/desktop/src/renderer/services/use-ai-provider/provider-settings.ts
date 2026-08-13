/**
 * Pure field resolution for `ai_set_provider_settings`.
 *
 * The command is **PATCH, per field**: an omitted field keeps what is stored,
 * an explicit `null` clears it, a value sets it. So a caller sends only what it
 * is changing — with ONE exception this function exists to enforce.
 *
 * **The window belongs to the model.** A stored `contextWindow` describes the
 * model that was stored with it, so a save that CHANGES the model must also say
 * what happens to the window: the new model's own window if the user has set
 * one, otherwise an explicit `null`. Leaving it out would silently run the new
 * model at the old model's `num_ctx` — the quietest of the failure modes,
 * because Settings would still show the right numbers.
 *
 * Kept separate from the hook so the rule is unit-testable without React Query.
 */

import type { AiProviderRouting } from '@ajh/shared';

/** The `modelLimits` shape the renderer persists per local model. Only the
 *  window matters here; `maxTokens`/`temperature` never reach the backend. */
export type LocalModelWindows = Record<string, { contextWindow?: number } | undefined>;

export interface ProviderSettingsWriteInput {
  provider: string;
  /** The row as the backend holds it today (`activeConfig.providers[provider]`).
   *  Read ONLY to tell whether the model is changing. */
  stored?: AiProviderRouting;
  /** The model to save. Absent = not changing it. */
  model?: string;
  /** `null` CLEARS the stored base URL; absent leaves it alone. (Only
   *  `openai-compatible` persists one — the backend nulls it for the rest.) */
  baseUrl?: string | null;
  /** An explicitly chosen window (the Settings slider). Wins over the per-model
   *  limit; `null` clears the stored one. */
  contextWindow?: number | null;
  /** The renderer's `providers.ollama.modelLimits` — the only surface on which
   *  a user sets a window, keyed by model name. */
  localWindows?: LocalModelWindows;
}

/**
 * The patch to send: only the keys that are actually changing.
 *
 * `model` is NOT nullable here even though the command accepts a clear: no
 * renderer surface removes a model (you pick a different one), so the input
 * type cannot express it either — an output state nothing can produce is an
 * untestable branch, not a feature.
 */
export interface ProviderSettingsWrite {
  provider: string;
  model?: string;
  baseUrl?: string | null;
  contextWindow?: number | null;
}

/** Build the patch for one provider row — see the module doc for the one rule
 *  that is not "pass the caller's intent through". */
export function resolveProviderSettingsWrite(
  input: ProviderSettingsWriteInput
): ProviderSettingsWrite {
  const { provider, stored, localWindows } = input;
  const write: ProviderSettingsWrite = { provider };

  if (input.model !== undefined) write.model = input.model;
  if (input.baseUrl !== undefined) write.baseUrl = input.baseUrl;

  if (input.contextWindow !== undefined) {
    write.contextWindow = input.contextWindow;
  } else if (input.model !== undefined && input.model !== stored?.model) {
    // The model is changing, so the stored window no longer describes it.
    const ownWindow = localWindows?.[input.model]?.contextWindow;
    if (ownWindow !== undefined) {
      // The new model has a window of its own — send it either way.
      write.contextWindow = ownWindow;
    } else if (stored !== undefined) {
      // Clear it — but ONLY against a row we have actually read. An unresolved
      // `activeConfig` query also reads as "no stored row", and treating that
      // as "the model changed" sent `contextWindow: null` on an unchanged model
      // and wiped a real window. Unknown means send nothing, which patch
      // semantics read as "keep".
      write.contextWindow = null;
    }
  }

  return write;
}
