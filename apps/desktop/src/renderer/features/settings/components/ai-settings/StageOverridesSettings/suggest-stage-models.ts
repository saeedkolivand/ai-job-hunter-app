/**
 * The one suggestion this surface makes: run the EXTRACTION stages on a smaller
 * model that is already installed.
 *
 * Pure and never applied on the user's behalf — the caller renders it as a
 * suggestion with an explicit accept, exactly like `suggestLocalLimits` +
 * "Use suggested".
 *
 * **Both sides must be MEASURED.** An earlier version ranked models with
 * `getModelTier`, whose name heuristic answers `small` for anything it cannot
 * parse — so it recommended `llama3.3:latest` (70B) as "smaller" than an 8B
 * model, and `all-minilm` (an embedding model) for three JSON chat stages.
 * Silence is the correct output when the data is missing: this suggestion is
 * unsolicited advice, and unsolicited advice has to be right.
 */

import type { AiStageOverride, PipelineStage } from '@ajh/shared';
import type { ModelInspectResult } from '@ajh/shared/schemas';

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

/**
 * How much smaller the candidate must be before it is worth suggesting.
 * A 7B-vs-8B swap is churn, not an improvement.
 */
const MIN_SIZE_RATIO = 2;

/** Below this the model is too small to be trusted with the JSON contract the
 *  extraction stages depend on (the same tier boundary `getModelTier` uses for
 *  the small-model warning). */
const MIN_CANDIDATE_B = 1;

export interface StageSuggestionInput {
  /** The active provider id; only local Ollama is suggested for. */
  activeProvider?: string;
  activeModel?: string;
  /** Installed local model names (`/api/tags`), in the order Ollama listed them. */
  installedModels: string[];
  /** `/api/show` per model — the MEASURED parameter size and family. A model
   *  missing from here (or with no `parameterSize`) is never a candidate and
   *  never a baseline: unknown is not "small". */
  inspections: Record<string, ModelInspectResult | null | undefined>;
  /** Stages already pinned by the user are never re-suggested. */
  overrides: Record<string, AiStageOverride>;
}

export interface StageSuggestion {
  /** The installed model to point the stages at. */
  model: string;
  /** Only the extraction stages that have no override yet. */
  stages: PipelineStage[];
  /** Measured parameter counts, in billions — what the copy quotes so the user
   *  can check the claim rather than trust it. */
  candidateB: number;
  activeB: number;
}

/**
 * Parse Ollama's reported `parameter_size` ("8.0B", "1.2B", "596.05M") into
 * billions. Returns `null` for anything unrecognised — the whole point is that
 * an unparsed size disqualifies a model instead of defaulting it.
 */
export function parseParameterSizeB(size: string | undefined): number | null {
  if (!size) return null;
  const match = /^\s*([\d.]+)\s*([BbMmKk])\s*$/.exec(size);
  if (!match) return null;
  const value = Number.parseFloat(match[1] ?? '');
  if (!Number.isFinite(value)) return null;
  const unit = (match[2] ?? 'b').toLowerCase();
  if (unit === 'b') return value;
  if (unit === 'm') return value / 1_000;
  return value / 1_000_000;
}

/**
 * Is this model able to hold a chat/JSON turn at all?
 *
 * Answered from the INSPECTION where possible — an embedding model reports an
 * embedding family (`bert`, `nomic-bert`, …) — because the name is not
 * reliable: `all-minilm` contains no "embed" substring at all, which is exactly
 * how it got recommended for three chat stages.
 */
export function isChatCapable(model: string, info: ModelInspectResult | null | undefined): boolean {
  const family = info?.family?.toLowerCase() ?? '';
  if (family) {
    // Embedding architectures, as Ollama reports them.
    return !/bert|embed|minilm|e5|gte|bge/.test(family);
  }
  // No family reported — fall back to the name, and only to REJECT.
  return !/embed|minilm|bge|gte-|e5-/i.test(model);
}

/**
 * Suggest a smaller installed model for the extraction stages, or `null` when
 * there is nothing honest to suggest.
 *
 * `null` when: the active provider is not local Ollama, either side has no
 * MEASURED parameter size, no candidate is at least {@link MIN_SIZE_RATIO}×
 * smaller, the only candidates are embedding models or below
 * {@link MIN_CANDIDATE_B}, or all three stages are already pinned.
 */
export function suggestExtractionModel(input: StageSuggestionInput): StageSuggestion | null {
  const { activeProvider, activeModel, installedModels, inspections, overrides } = input;
  if (activeProvider !== 'ollama' || !activeModel) return null;

  const stages = EXTRACTION_STAGES.filter((stage) => !overrides[stage]);
  if (stages.length === 0) return null;

  // The BASELINE must be measured too: without it there is no "smaller".
  const activeB = parseParameterSizeB(inspections[activeModel]?.parameterSize);
  if (activeB === null) return null;

  const candidates = installedModels
    .filter((name) => name !== activeModel)
    .map((name) => ({
      name,
      info: inspections[name],
      b: parseParameterSizeB(inspections[name]?.parameterSize),
    }))
    .filter(
      (c): c is { name: string; info: ModelInspectResult | null | undefined; b: number } =>
        c.b !== null && c.b >= MIN_CANDIDATE_B && c.b * MIN_SIZE_RATIO <= activeB
    )
    .filter((c) => isChatCapable(c.name, c.info));
  if (candidates.length === 0) return null;

  // The LARGEST qualifying candidate: it is already comfortably smaller than the
  // active model, and among the ones that clear the bar the biggest is the
  // safest for a stage that still has to produce valid JSON.
  const best = candidates.reduce((a, b) => (b.b > a.b ? b : a));

  return { model: best.name, stages: [...stages], candidateB: best.b, activeB };
}
