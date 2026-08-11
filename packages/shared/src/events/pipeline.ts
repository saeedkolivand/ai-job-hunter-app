export const PIPELINE_EVENTS = {
  stage: 'pipeline:stage',
} as const;

/**
 * Which half of a stage's lifecycle this event reports.
 * - `start` — the stage is about to run (emitted from the hook's `before`).
 * - `finish` — it completed successfully.
 * - `error` — it failed; the run aborts after this event.
 */
export type PipelineStagePhase = 'start' | 'finish' | 'error';

/**
 * Payload of the `pipeline:stage` event — the per-stage progress trail of one
 * multi-step run (Rust `PipelineStageEvent`, camelCase).
 *
 * Emitted by the Phase-3 shell hook that implements the Rust `StageHooks`
 * trait; the channel and this shape are frozen now so the contract is stable
 * before either side is written. Content-free by construction (ADR-027): stage
 * names, counts, and durations only — never generated text.
 */
export interface PipelineStageEvent {
  /** The pipeline run this stage belongs to (`pipeline_runs.id`). */
  runId: string;
  /** The job/run id the renderer already filters `jobs:event` on, so a panel
   *  can correlate stage progress with the job it started. */
  jobId: string;
  /** The stage's stable name (Rust `Stage::name`), e.g. `"draft"`. */
  stage: string;
  phase: PipelineStagePhase;
  /** 0-based position of this stage in the pipeline. */
  index: number;
  /** How many stages the pipeline has in total — `index`/`total` drives the
   *  progress bar without the renderer knowing the stage list. */
  total: number;
  /** 1-based attempt number for this stage: `1` on the first run, `2`+ for a
   *  budgeted repair re-ask (see the Rust `Budget::max_repair_attempts`). */
  attempt: number;
  /** The résumé section this stage worked on, for section-wise stages only.
   *  Omitted from the wire otherwise. */
  sectionKey?: string;
  /** Wall-clock duration of the stage body. Present on `finish`/`error` only. */
  ms?: number;
  /** Total content-validation issues found by this stage. `finish`/`error` only. */
  issueCount?: number;
  /** How many of `issueCount` were CRITICAL — the subset that blocks. */
  criticalCount?: number;
}
