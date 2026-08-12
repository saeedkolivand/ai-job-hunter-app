/**
 * The one suggestion this surface makes: run the EXTRACTION stages on a smaller
 * model that is already installed.
 *
 * Pure and never applied on the user's behalf — the caller renders it as a
 * suggestion with an explicit accept, exactly like `suggestLocalLimits` +
 * "Use suggested". Writing it silently would pin three stages to today's model
 * forever (an absent override row means "follow the active provider").
 */

import { getModelTier } from '@ajh/prompts/context-manager';
import type { AiStageOverride, PipelineStage } from '@ajh/shared';

/**
 * The three stages that only READ (the posting, the résumé) and answer in
 * strict JSON — no prose is written here, so model size buys much less than it
 * does in `draft`/`sections`. Deliberately not derived from the depth lists: it
 * is a claim about what these stages DO, which is why each is named.
 */
export const EXTRACTION_STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
] as const satisfies readonly PipelineStage[];

/** Smallest first — the ordering "is this candidate smaller" is decided on. */
const TIER_ORDER = ['small', 'medium', 'large'] as const;

export interface StageSuggestionInput {
  /** The active provider id; only local Ollama is suggested for. */
  activeProvider?: string;
  activeModel?: string;
  /** Installed local model names (`/api/tags`), in the order Ollama listed them. */
  installedModels: string[];
  /** Stages already pinned by the user are never re-suggested. */
  overrides: Record<string, AiStageOverride>;
}

export interface StageSuggestion {
  /** The installed model to point the stages at. */
  model: string;
  /** Only the extraction stages that have no override yet. */
  stages: PipelineStage[];
}

/** An embedding model can't answer a chat/JSON turn — never suggest one. */
function isChatModel(name: string): boolean {
  return !/embed/i.test(name);
}

/**
 * Suggest a smaller installed model for the extraction stages, or `null` when
 * there is nothing honest to suggest.
 *
 * `null` when: the active provider is not local Ollama (a cloud model's cost is
 * not fixed by swapping in a local one), the active model is unknown, no
 * installed model is in a strictly smaller size tier (the SAME `getModelTier`
 * that already picks the prompt tier — never a second table), or all three
 * stages are already pinned. Among equally-small candidates the first in
 * Ollama's own listing order wins, so the suggestion is stable across renders.
 */
export function suggestExtractionModel(input: StageSuggestionInput): StageSuggestion | null {
  const { activeProvider, activeModel, installedModels, overrides } = input;
  if (activeProvider !== 'ollama' || !activeModel) return null;

  const stages = EXTRACTION_STAGES.filter((stage) => !overrides[stage]);
  if (stages.length === 0) return null;

  const activeRank = TIER_ORDER.indexOf(getModelTier(activeModel));
  const candidates = installedModels.filter(
    (name) =>
      name !== activeModel &&
      isChatModel(name) &&
      TIER_ORDER.indexOf(getModelTier(name)) < activeRank
  );
  if (candidates.length === 0) return null;

  const smallest = candidates.reduce((best, name) =>
    TIER_ORDER.indexOf(getModelTier(name)) < TIER_ORDER.indexOf(getModelTier(best)) ? name : best
  );

  return { model: smallest, stages: [...stages] };
}
