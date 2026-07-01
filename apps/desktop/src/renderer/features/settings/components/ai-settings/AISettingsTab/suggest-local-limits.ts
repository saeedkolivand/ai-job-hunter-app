/**
 * Pure heuristics for suggesting safe local-model (Ollama) generation limits from
 * the model's real context length (`/api/show`) and the machine's free memory.
 * Kept separate from the UI so it is unit-testable.
 */

export interface LimitSuggestion {
  /** Suggested context window (num_ctx), in tokens. */
  contextWindow: number;
  /** Suggested max output (num_predict), in tokens. */
  maxTokens: number;
}

export interface SuggestInput {
  /** Model's trained max context from `/api/show`, if known. */
  modelMaxContext?: number;
  /** Free system RAM in GB. */
  freeRamGb: number;
  /** Whether a GPU is present (KV cache then lives in VRAM). */
  hasGpu: boolean;
  /** Free VRAM in GB (0 when no GPU). */
  freeVramGb: number;
}

/** Schema bounds — keep in lockstep with `LocalModelLimitsSchema`. */
const CTX_FLOOR = 2048;
const CTX_CEIL = 131072;
const OUT_FLOOR = 512;
const OUT_CEIL = 8192;

/** Rough KV-cache budget for a typical 7–8B GQA model. */
const TOKENS_PER_GB = 8192;
const HEADROOM_GB = 1;

/**
 * Suggest a context window that fits the available memory (capped at the model's
 * trained max) plus a sensible max output.
 *
 * KV-cache memory scales ~linearly with `num_ctx`. We budget ~8K tokens per GB of
 * free memory (VRAM when a GPU is present, else system RAM), keep ~1 GB headroom,
 * and never exceed the model's trained context length — so the suggestion **caps
 * at the model max** and **drops on low-memory machines**.
 */
export function suggestLocalLimits(input: SuggestInput): LimitSuggestion {
  const { modelMaxContext, freeRamGb, hasGpu, freeVramGb } = input;

  const modelCap = modelMaxContext && modelMaxContext > 0 ? modelMaxContext : OUT_CEIL;
  const budgetGb = Math.max(0, hasGpu ? freeVramGb : freeRamGb);
  const tokensFromMemory = Math.max(0, budgetGb - HEADROOM_GB) * TOKENS_PER_GB;

  // Fit memory, cap at the model max, then clamp to the schema range and round to
  // a 512-token step.
  let contextWindow = Math.min(modelCap, tokensFromMemory) || CTX_FLOOR;
  contextWindow = Math.max(CTX_FLOOR, Math.min(CTX_CEIL, contextWindow));
  contextWindow = Math.round(contextWindow / 512) * 512;

  // Output budget: a slice of the context, within sane bounds.
  const maxTokens = Math.max(OUT_FLOOR, Math.min(OUT_CEIL, Math.round(contextWindow / 4)));

  return { contextWindow, maxTokens };
}
