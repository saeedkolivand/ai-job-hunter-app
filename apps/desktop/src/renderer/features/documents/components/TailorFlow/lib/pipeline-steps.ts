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

// KNOWN GAP, currently unreachable: a MAX-depth run's `sections`/`assemble`/
// `llm_judge` stages have no entry here (they fall through to "stay where you
// are" below), and max depth has no `humanize` stage at all — so this 4-step
// UI would mis-narrate a max run (stuck checklist, a "Remove AI signs" step
// that never happens). Not reachable today: the wizard's depth picker was
// removed and every run defaults to quality depth. PR-4 deletes max depth
// outright, so this is left as a comment rather than a speculative fix.
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
