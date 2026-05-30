/** Model-size detection from a model name / Ollama tag. */

import {
  LARGE_MODEL_STRATEGY,
  MEDIUM_MODEL_STRATEGY,
  SMALL_MODEL_STRATEGY,
  type TruncationStrategy,
} from './truncation.js';

/**
 * Detect model size from a model name / Ollama tag.
 *
 * This is purely about model **size** for the ollama sub-tier and the back-compat
 * name→tier path. The **provider kind** (ollama / cloud / cli) is decided by the
 * provider layer (`resolveProfile`), not here — a CLI agent gets the task brief by
 * its `kind`, regardless of size. We still recognise hosted + CLI-agent model
 * names (incl. Claude Code's sonnet/opus/haiku and codex) as `large` so they get
 * the full prompt rather than falling through to the small default.
 *
 * For local models the **parameter size** is parsed generically from the tag
 * (`:1b`, `-3.2-1b`, `:7b`, `70b`, with quant / `-instruct` suffixes) →
 * `<4B small · 4–14B medium · >14B large`. An unrecognised LOCAL model (no size,
 * not a known hosted name) defaults to the smaller/safer `small` prompt.
 */
export function detectModelSize(modelName: string): 'large' | 'medium' | 'small' {
  const name = modelName.toLowerCase();

  // Hosted cloud + CLI-agent models — capable, always the full prompt.
  if (
    name.includes('gpt-') ||
    name.includes('gpt4') ||
    /\bo[134]\b/.test(name) ||
    name.includes('claude') ||
    name.includes('sonnet') ||
    name.includes('opus') ||
    name.includes('haiku') ||
    name.includes('codex') ||
    name.includes('gemini') ||
    name.includes('command-r') ||
    name.includes('openai') ||
    name.includes('anthropic') ||
    name.includes('mistral-large') ||
    name.includes('mixtral')
  ) {
    return 'large';
  }

  const size = parseParamSize(name);
  if (size !== null) {
    if (size < 4) return 'small';
    if (size <= 14) return 'medium';
    return 'large';
  }

  // Unknown local model — safer to under-prompt than to over-prompt a tiny model.
  return 'small';
}

/**
 * Parse the parameter count (in billions) from a model tag. Normalizes separators
 * so `llama3.2:1b`, `llama-3.2-1b`, and `qwen2.5:0.5b` all parse, and ignores
 * version tokens (the `3` in `llama3`) and quant suffixes (`-q4`, `:q4_K_M`).
 */
function parseParamSize(name: string): number | null {
  const normalized = name.replace(/[_:]/g, '-');
  // A number directly followed by 'b' (billions), bounded so quant codes and
  // version numbers don't match: e.g. 0.5b, 1b, 7b, 70b.
  const matches = [...normalized.matchAll(/(?:^|[^0-9.])(\d+(?:\.\d+)?)\s*b(?![a-z0-9])/g)];
  const sizes = matches.map((m) => parseFloat(m[1] ?? '')).filter((n) => !Number.isNaN(n));
  return sizes.length ? Math.max(...sizes) : null;
}

/**
 * Public alias for {@link detectModelSize}. Use this in prompt builders to select
 * the appropriate prompt tier.
 */
export function getModelTier(modelName: string): 'large' | 'medium' | 'small' {
  return detectModelSize(modelName);
}

/** Get the matching truncation strategy for a model name. */
export function getStrategyForModel(modelName: string): TruncationStrategy {
  const size = detectModelSize(modelName);

  switch (size) {
    case 'large':
      return LARGE_MODEL_STRATEGY;
    case 'medium':
      return MEDIUM_MODEL_STRATEGY;
    case 'small':
      return SMALL_MODEL_STRATEGY;
    default:
      return MEDIUM_MODEL_STRATEGY;
  }
}
