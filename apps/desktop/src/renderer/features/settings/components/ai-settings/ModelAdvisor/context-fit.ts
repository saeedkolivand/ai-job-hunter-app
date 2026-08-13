/**
 * Does a model's context window hold what a stage actually sends it?
 *
 * The DEMAND side is the Rust prompt builders' own char caps — the only honest
 * bound available, since nothing measures a real prompt at runtime. Each stage
 * below sums exactly the fenced blocks its builder emits
 * (`pipeline/resume/prompts.rs`, `section_prompts.rs`), so this is the
 * worst case the fence permits, not a guess.
 *
 * The fence truncates with NO marker, so a model that overflows does not fail —
 * it reads a fragment as if it were the whole input. That is why this is worth
 * showing before a run rather than after one.
 */

import { estimateTokens } from '@ajh/prompts';
import type { PipelineStage } from '@ajh/shared';

/** Char caps, verbatim from the Rust constants that enforce them. */
const ARTIFACT = 16_000; // prompts.rs ARTIFACT_CAP — one serialized prior-stage artifact
const RESUME = 8_000; // agent/tools.rs RESUME_CAP
const JOB = 8_000; // agent/tools.rs JOB_CAP
const NOTE = 500; // prompts.rs NOTE_CAP
const SECTION = 4_000; // prompts.rs SECTION_CAP
const DOCUMENT = 12_000; // section_prompts.rs DOCUMENT_CAP — what the judge reads

/**
 * Worst-case prompt INPUT per stage, in characters — the sum of that stage's
 * own fenced blocks:
 *
 * - `analyze_job`    job_posting
 * - `match_evidence` candidate_resume + job_analysis
 * - `strategy`       candidate_resume + job_analysis + evidence_map
 * - `draft`          candidate_resume + job_posting + resume_strategy (the
 *                    ~32 k figure the Rust doc comment states)
 * - `sections`       source_entry + job_analysis + resume_strategy slice +
 *                    evidence_map + project_seed (projects only) + section_note
 * - `repair`         candidate_resume + resume_section + section_issues + note
 * - `llm_judge`      generated_resume + job_posting + job_analysis
 *
 * Every artifact term is charged at its full cap even though the measured worst
 * cases sit well under it (evidence_map 15 199 chars, strategy 7 553) — this is
 * a ceiling, and the UI says so rather than presenting it as what a run costs.
 */
export const STAGE_WORST_CASE_CHARS: Partial<Record<PipelineStage, number>> = {
  analyze_job: JOB,
  match_evidence: RESUME + ARTIFACT,
  strategy: RESUME + ARTIFACT * 2,
  draft: RESUME + JOB + ARTIFACT,
  sections: SECTION + ARTIFACT * 4 + NOTE,
  repair: RESUME + SECTION + ARTIFACT + NOTE,
  llm_judge: DOCUMENT + JOB + ARTIFACT,
  // PARTIAL on purpose: the free stages (`assemble`, `validate`) send no prompt
  // at all. Keyed by `PipelineStage` all the same, so a stage renamed upstream
  // is a compile error here rather than a silently dead entry.
};

/**
 * The chars-per-token ratio, read OUT of the shared estimator instead of
 * re-declaring `CHARS_PER_TOKEN` here — the caps are characters and a window is
 * tokens, so something has to convert, and two copies of that number are two
 * numbers that can drift. Sampled once at module load.
 *
 * The default (English) ratio is used deliberately: a denser source language
 * yields MORE tokens per char, so the headroom below absorbs it rather than the
 * estimate quietly under-reporting per locale.
 */
const RATIO_SAMPLE = 10_000;
const CHARS_PER_TOKEN = RATIO_SAMPLE / estimateTokens('x'.repeat(RATIO_SAMPLE));

/** Tokens the answer needs on top of the input — the app's own default max
 *  output (`LocalModelLimits`' `maxTokens ?? 2048`). */
export const ANSWER_HEADROOM_TOKENS = 2048;

/**
 * The defensible floor from the prompt-cap analysis: ~8 k tokens of input.
 * Below this, the largest single turn (`draft`) is being cut, so the model is
 * not merely tight — it is reading fragments.
 */
export const MIN_USABLE_INPUT_TOKENS = 8_000;

export function estimateTokensFromChars(chars: number): number {
  return Math.ceil(chars / CHARS_PER_TOKEN);
}

export type ContextFitVerdict =
  /** Holds every stage's worst case. */
  | 'fits'
  /** Holds a normal run, but at least one stage's worst case would be cut. */
  | 'tight'
  /** Below the floor — stages will be truncated with no marker. */
  | 'too-small'
  /** No `/api/show` answer for this model. NOT the same as "too small". */
  | 'unknown';

export interface ModelContextFit {
  model: string;
  /** The model's trained window, when Ollama reported one. */
  contextLength?: number;
  /** Window minus the answer headroom — what the prompt actually gets. */
  usableInputTokens?: number;
  verdict: ContextFitVerdict;
  /** Stages whose worst-case prompt does not fit, named so the UI can say
   *  WHICH one rather than "some stage". */
  overflowStages: PipelineStage[];
}

export interface ContextFitInput {
  model: string;
  /** From `ai_inspect_model`; absent for a non-local provider or an unreachable
   *  Ollama — absence means NOT MEASURED and must render that way. */
  contextLength?: number;
  /** Defaults to every stage in {@link STAGE_WORST_CASE_CHARS}. */
  stages?: readonly PipelineStage[];
}

/**
 * The stage with the largest worst-case prompt — COMPUTED, because the answer
 * is not the obvious one: `sections` carries four artifacts and is 2.1× the
 * draft turn, so naming `draft` in the copy (as an early version did) was
 * simply false against this module's own table.
 */
export function worstCaseStage(): { stage: PipelineStage; chars: number; tokens: number } {
  const [stage, chars] = Object.entries(STAGE_WORST_CASE_CHARS).reduce((a, b) =>
    b[1] > a[1] ? b : a
  );
  return {
    stage: stage as PipelineStage,
    chars,
    tokens: estimateTokensFromChars(chars),
  };
}

/** Fit verdict for ONE model against the stage budgets. */
export function assessModelContextFit(input: ContextFitInput): ModelContextFit {
  const { model, contextLength } = input;
  const stages = input.stages ?? (Object.keys(STAGE_WORST_CASE_CHARS) as PipelineStage[]);

  if (!contextLength || contextLength <= 0) {
    return { model, verdict: 'unknown', overflowStages: [] };
  }

  // Clamped: a window smaller than the answer budget leaves ZERO for the
  // prompt, not a negative amount of it.
  const usableInputTokens = Math.max(0, contextLength - ANSWER_HEADROOM_TOKENS);
  const overflowStages = stages.filter(
    (stage) => usableInputTokens < estimateTokensFromChars(STAGE_WORST_CASE_CHARS[stage] ?? 0)
  );

  const verdict: ContextFitVerdict =
    usableInputTokens < MIN_USABLE_INPUT_TOKENS
      ? 'too-small'
      : overflowStages.length > 0
        ? 'tight'
        : 'fits';

  return { model, contextLength, usableInputTokens, verdict, overflowStages };
}
