import { getModelTier } from '@ajh/prompts/context-manager';

import { useGenerateConfig } from '@/services';

/**
 * Does the active model look small AND local?
 *
 * The staged pipeline leans on JSON-following: three of its stages parse a
 * structured answer, and a model that can't hold the shape burns its one
 * budget-charged re-ask and then degrades the run to needs-review. Small local
 * models are where that actually bites (2026 research: nested schemas push
 * ~68% non-compliance on this tier), so the surface says so — as a WARNING,
 * never a block. Parse failures degrade honestly; they don't corrupt anything.
 *
 * "Local" is `provider === 'ollama'`: cloud and CLI providers get the full
 * prompt and are not the failure mode this warns about (and ollama-cloud is a
 * hosted endpoint, not the user's GPU). "Small" is the shared model-name
 * heuristic — the SAME `getModelTier` that already picks the prompt tier, not a
 * second table that could disagree with it.
 */
export function useSmallModelWarning(): boolean {
  const { provider, model } = useGenerateConfig();
  if (provider !== 'ollama' || !model) return false;
  return getModelTier(model) === 'small';
}
