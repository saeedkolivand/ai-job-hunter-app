/**
 * The FOUR visible steps the apply-flow stepper renders, folded from the
 * Rust quality pipeline's eight `pipeline:stage` names (PR-3 §6 of the
 * staged-cutover plan).
 *
 * Mirrors the posture of `resume-pipeline.machine`'s own `stageToEvent`: a
 * stage name this build doesn't know (a future stage, or one of max depth's
 * `sections`/`assemble`/`llm_judge`) keeps the step where it already is
 * rather than regressing the checklist — forward-compat by construction, not
 * by a list that has to be kept in sync with the Rust stage list by hand.
 */
export const PIPELINE_STEP_KEYS = ['analyze', 'generate', 'validate', 'humanize'] as const;
export type PipelineStepKey = (typeof PIPELINE_STEP_KEYS)[number];

const STEP_INDEX: Record<string, number> = {
  analyze_job: 0,
  match_evidence: 0,
  strategy: 0,
  draft: 1,
  cover_letter: 1,
  validate: 2,
  repair: 2,
  humanize: 3,
};

/**
 * Which of the {@link PIPELINE_STEP_KEYS} a raw `pipeline:stage` name belongs
 * to. `previous` is the step index the caller was already showing — returned
 * unchanged for a stage this map doesn't recognise, so an unknown stage never
 * moves the checklist backward (or anywhere at all).
 */
export function pipelineStepForStage(stage: string, previous: number): number {
  return STEP_INDEX[stage] ?? previous;
}
