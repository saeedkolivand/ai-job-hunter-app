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
  /** The provider reported a ratio and it is FINE. The one state here that is
   *  actual evidence of low risk — never conflate it with `unmeasured`, whose
   *  copy explains that nothing was measured at all. */
  | 'measured-light'
  /** A thinking-heavy family AND the effort level the loop reports cluster at. */
  | 'known-bad-combo'
  /** A thinking-heavy family at a lower effort — worth knowing, not alarming. */
  | 'reasoning-heavy'
  /** Nothing measured and no name match. NOT a clean bill of health. */
  | 'unmeasured';

export interface ThinkingRisk {
  model: string;
  level: ThinkingRiskLevel;
  /** thinking ÷ output tokens, for either MEASURED level. Absent when the
   *  provider reported thinking against zero output, where no ratio exists. */
  ratio?: number;
  /** Sample size behind `ratio`. */
  calls?: number;
  /** The effort level this assessment was made against. */
  effort?: string;
}

export interface ThinkingRiskInput {
  model: string;
  /** The provider this model will actually run on, when known. Model ids are
   *  not globally unique — `gpt-oss:20b` on Ollama and the same name behind an
   *  OpenAI-compatible endpoint are different rows — so a measurement is only
   *  this model's when the provider matches too. */
  provider?: string;
  /** The provider's configured reasoning effort, when it has one. */
  effort?: string;
  /** `spendSummary().thinkingByModel` — empty for a local-only user. */
  measured?: AiSpendModelThinking[];
}

export function assessThinkingRisk(input: ThinkingRiskInput): ThinkingRisk {
  const { model, provider, effort, measured = [] } = input;

  const row = measured.find(
    (m) => m.model === model && (provider === undefined || m.provider === provider)
  );
  if (row && row.calls >= MEASURED_MIN_CALLS) {
    if (row.outputTokens > 0) {
      const ratio = row.thinkingTokens / row.outputTokens;
      return {
        model,
        level: ratio >= HEAVY_RATIO ? 'measured-heavy' : 'measured-light',
        ratio,
        calls: row.calls,
        effort,
      };
    }
    // All thinking, no answer: the worst version of the failure, not a missing
    // measurement. A ratio is undefined here, so the level carries the meaning.
    if (row.thinkingTokens > 0) {
      return { model, level: 'measured-heavy', calls: row.calls, effort };
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
