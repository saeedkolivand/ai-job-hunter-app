/**
 * Rust `StoppedReason` (`pipeline::budget`, `#[serde(rename_all = "snake_case")]`)
 * → the i18n key SUFFIX a surface appends to its own namespace.
 *
 * One map for every `pipeline.stopped.*`-style label. The enum is shared, so
 * the mapping is too — a variant added to `pipeline::budget` needs ONE entry
 * here, not one per surface, and a surface that forgets it lands on
 * {@link UNKNOWN_STOPPED_SUFFIX} rather than on a wrong-but-plausible label.
 *
 * `timeout` is deliberately absent: the staged pipeline turns it into a job
 * FAILURE, so it surfaces as an error state and never reaches a stopped tag.
 */
export type StoppedSuffix =
  | 'done'
  | 'maxSteps'
  | 'maxTokens'
  | 'cancelled'
  | 'truncated'
  | 'budgeted'
  | 'runTimeout'
  | 'maxToolCalls'
  | 'maxRepairs';

export const STOPPED_SUFFIX: Record<string, StoppedSuffix> = {
  done: 'done',
  max_steps: 'maxSteps',
  max_tokens: 'maxTokens',
  cancelled: 'cancelled',
  truncated: 'truncated',
  budgeted: 'budgeted',
  run_timeout: 'runTimeout',
  max_tool_calls: 'maxToolCalls',
  max_repairs: 'maxRepairs',
};

/**
 * Label suffix for a reason this build doesn't know — a variant added to the
 * Rust enum after this renderer shipped.
 *
 * It must NEVER be `done`: a `…stopped.done` key exists on both namespaces, so
 * an i18n `defaultValue` can't rescue an unmapped reason, and a timed-out or
 * budget-exhausted run would render as "Completed". Vague beats wrong.
 */
export const UNKNOWN_STOPPED_SUFFIX = 'stopped';

/**
 * The suffix for one wire reason, including the two cases the STAGED pipeline
 * adds that the agent flow never had:
 *
 * - **`null`/`undefined` on a terminal run.** `PipelineRunSummary.stoppedReason`
 *   is nullable by contract: a run that failed without recording a reason
 *   carries none. `'done'` is never used as a fallback for it — `status` is the
 *   authority on how a run ended, and a missing reason means "no further
 *   detail", not "finished cleanly".
 * - **An empty string**, which some stores round-trip in place of `NULL`.
 */
export function stoppedSuffix(
  reason: string | null | undefined
): StoppedSuffix | typeof UNKNOWN_STOPPED_SUFFIX | null {
  if (reason == null || reason.trim() === '') return null;
  return STOPPED_SUFFIX[reason] ?? UNKNOWN_STOPPED_SUFFIX;
}
