/**
 * "Will this model think itself to death on this setting?"
 *
 * Two independent signals, deliberately not collapsed into one:
 *
 * 1. MEASURED — `spendSummary().thinkingByModel`, the provider's own reported
 *    reasoning-token split. Only OpenAI and Gemini report it; Anthropic counts
 *    thinking inside its output tokens and Ollama's `eval_count` includes the
 *    thinking channel, so a LOCAL-ONLY user's list is permanently EMPTY.
 * 2. NAME — the thinking-heavy families, which is the only signal available for
 *    exactly the case that motivated this (a `gpt-oss:20b` run logged a 22.9:1
 *    reasoning-to-answer ratio and took 8–13 minutes per document where a
 *    non-reasoning model took 12–23 seconds).
 *
 * Absence of measurement is therefore reported as `unmeasured`, never as
 * "fine": no row here means the provider does not report the split, not that
 * the model does not reason.
 */

import type { AiSpendModelThinking } from '@ajh/shared';

/** Thinking-heavy model families. Substring matches on the model id, which is
 *  what a user picks — a name is a weak signal, so it drives a WARNING and
 *  never a block. */
const REASONING_HEAVY_PATTERNS = [
  'gpt-oss',
  'deepseek-r1',
  'qwq',
  'magistral',
  'thinking',
  'reasoner',
];

/** The effort level the loop reports cluster at. */
const RISKY_EFFORT = 'high';

/** Mirrors `GeneratingPanel`'s live char-ratio threshold — one number for
 *  "this model thinks far more than it answers", whichever unit measured it. */
export const HEAVY_RATIO = 5;

/** "One or two calls is noise" — the spend read model's own caveat. */
export const MEASURED_MIN_CALLS = 3;

export type ThinkingRiskLevel =
  /** The provider reported a heavy reasoning:answer ratio for this model. */
  | 'measured-heavy'
  /** A thinking-heavy family AND the effort level the loop reports cluster at. */
  | 'known-bad-combo'
  /** A thinking-heavy family at a lower effort — worth knowing, not alarming. */
  | 'reasoning-heavy'
  /** Nothing measured and no name match. NOT a clean bill of health. */
  | 'unmeasured';

export interface ThinkingRisk {
  model: string;
  level: ThinkingRiskLevel;
  /** Present only for `measured-heavy` — thinking ÷ output tokens. */
  ratio?: number;
  /** Sample size behind `ratio`. */
  calls?: number;
  /** The effort level this assessment was made against. */
  effort?: string;
}

export interface ThinkingRiskInput {
  model: string;
  /** The provider's configured reasoning effort, when it has one. */
  effort?: string;
  /** `spendSummary().thinkingByModel` — empty for a local-only user. */
  measured?: AiSpendModelThinking[];
}

export function assessThinkingRisk(input: ThinkingRiskInput): ThinkingRisk {
  const { model, effort, measured = [] } = input;

  const row = measured.find((m) => m.model === model);
  if (row && row.calls >= MEASURED_MIN_CALLS && row.outputTokens > 0) {
    const ratio = row.thinkingTokens / row.outputTokens;
    if (ratio >= HEAVY_RATIO) {
      return { model, level: 'measured-heavy', ratio, calls: row.calls, effort };
    }
  }

  const name = model.toLowerCase();
  if (REASONING_HEAVY_PATTERNS.some((p) => name.includes(p))) {
    return {
      model,
      level: effort === RISKY_EFFORT ? 'known-bad-combo' : 'reasoning-heavy',
      effort,
    };
  }

  return { model, level: 'unmeasured', effort };
}
