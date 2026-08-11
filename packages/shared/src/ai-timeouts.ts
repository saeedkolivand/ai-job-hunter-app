/**
 * Reasoning-effort → stream-timeout schedule — the SINGLE source of truth for
 * both sides of the streaming-generation deadline:
 *
 * - Backend: `apps/desktop/src-tauri/src/commands/ai_provider/timeouts.rs`
 *   (`stream_deadline`) consumes the generated
 *   `apps/desktop/src-tauri/src/ipc_contracts/ai_timeouts.rs` — emitted from
 *   this file by `packages/shared/scripts/gen-ipc-rust.ts` (`pnpm gen:ipc`).
 * - Renderer: `apps/desktop/src/renderer/lib/generate/stream-promise.ts`
 *   (`computeStreamTimeoutMs`) imports these constants directly.
 *
 * Before this file existed, Rust and TS each hand-mirrored these numbers
 * across the IPC boundary in independently-editable places — a change to one
 * side's copy could pass its own tests while breaking the invariant that
 * actually matters: the renderer's timeout for a given effort must always
 * strictly exceed the backend's own scaled deadline for that SAME effort (so
 * the backend's actionable provider error fires first, never the renderer's
 * generic "Generation timed out"). That invariant is asserted next to
 * `computeStreamTimeoutMs`, not here — this file only holds the numbers, and
 * `pnpm gen:ipc:check` is what keeps the Rust side from drifting off it.
 *
 * TIER ORDER, because it is not alphabetical and reads wrong at a glance:
 * `minimal < low < medium < high < xhigh < max`. **`max` is the TOP tier, not
 * `xhigh`** — per Anthropic's effort docs (`low | medium | high | xhigh |
 * max`) and OpenAI's `reasoning_effort` (`none | minimal | low | medium |
 * high | xhigh | max`). An earlier version of this schedule gave `max` a
 * SMALLER multiplier than `xhigh`, so the highest-effort requests — the ones
 * most likely to legitimately run long — got the SHORTEST deadline. Keep the
 * entries below in ascending tier order, and keep any monotonicity test's
 * tier array in that same order, or it will pass while pinning the
 * inversion right back.
 */

/** Baseline stream deadline, in seconds, for no/low/unrecognized effort. */
export const STREAM_BASELINE_SECS = 300;

/**
 * Effort tier → {@link STREAM_BASELINE_SECS} multiplier. Every tier NOT
 * listed here (`undefined`, `'minimal'`, `'low'`, or any unrecognized
 * string) gets an implicit 1.0 — no reason to extend the baseline for a tier
 * that isn't legitimately doing more reasoning work.
 */
export const EFFORT_TIMEOUT_MULTIPLIER: Readonly<Record<string, number>> = {
  medium: 1.5,
  high: 2.0,
  xhigh: 2.5,
  max: 3.0,
};

/** The multiplier for `effort`, with the same fallback both sides apply: any
 *  tier not in {@link EFFORT_TIMEOUT_MULTIPLIER} (absent, `minimal`, `low`, a
 *  typo, a future provider's name) is 1.0. */
function effortMultiplier(effort?: string): number {
  return (effort ? EFFORT_TIMEOUT_MULTIPLIER[effort] : undefined) ?? 1;
}

/**
 * The EFFORT-INVARIANT half of one quality-depth pipeline run's deadline, in
 * seconds — the three JSON stages (`analyze_job`, `match_evidence`,
 * `strategy`).
 *
 * Derived, not guessed. Each of those stages runs through
 * `Completer::complete_json`, which is allowed exactly ONE re-ask, and each
 * round-trip is bounded by the longest per-call HTTP timeout this app ships —
 * `timeouts::OLLAMA_COMPLETION`, 300 s (the local daemon's non-streaming
 * bound). That is 3 stages × 2 round-trips × 300 s = 1800 s. None of it scales
 * with reasoning effort, because `complete_with_usage`'s timeouts
 * (`COMPLETION` / `OLLAMA_COMPLETION`) are flat constants — only the STREAM
 * deadline is effort-scaled.
 *
 * The rule this exists to satisfy is the same one
 * `research_deadline_exceeds_the_inner_search_bounds_it_wraps` states for
 * research: **an outer bound that does not clear the inner bounds it wraps
 * becomes the binding constraint**, and the actionable inner error never gets
 * to fire. A run deadline below 1800 s would kill a run whose three JSON
 * stages were each answering, just slowly — on a local reasoning model, the
 * ordinary case.
 */
export const QUALITY_RUN_FIXED_SECS = 1_800;

/**
 * How many whole-document GENERATION passes one quality-depth run may make:
 * the draft, plus one per repair round (`Budget::max_repair_attempts` = 2). A
 * repair round regenerates only the failing sections, so it is bounded ABOVE by
 * one draft-equivalent, never below it.
 *
 * This is the term that scales with effort, because every one of those passes
 * is bounded by {@link STREAM_BASELINE_SECS} × the effort multiplier — the
 * exact bound `stream_deadline` enforces per call.
 */
export const QUALITY_RUN_GENERATION_PASSES = 3;

/**
 * Deadline (seconds) for ONE WHOLE quality-depth résumé pipeline run — the
 * backend's `StoppedReason::RunTimeout` trigger.
 *
 * `fixed + baseline × passes × multiplier(effort)`, i.e. the sum of the inner
 * per-call bounds the run can legitimately consume at that effort:
 *
 * | effort           | m   | JSON stages | generation passes | deadline        |
 * | ---------------- | --- | ----------- | ----------------- | --------------- |
 * | none/minimal/low | 1.0 | 1800 s      | 900 s             | 2700 s (45 min) |
 * | medium           | 1.5 | 1800 s      | 1350 s            | 3150 s (52 min) |
 * | high             | 2.0 | 1800 s      | 1800 s            | 3600 s (60 min) |
 * | xhigh            | 2.5 | 1800 s      | 2250 s            | 4050 s (67 min) |
 * | max              | 3.0 | 1800 s      | 2700 s            | 4500 s (75 min) |
 *
 * Deliberately NOT `baseline × multiplier`: half the run's cost does not scale
 * with effort at all (see {@link QUALITY_RUN_FIXED_SECS}), so a single
 * multiplicative constant either under-provisions the bottom tier — killing
 * legitimate runs — or wildly over-provisions the top one.
 *
 * This is a BACKSTOP for a run that never trips a per-step timeout but crawls
 * forever, not a target: the realistic clean quality run is +30–90 s over the
 * one-shot path. `Budget::step_timeout` is what catches a single hung call.
 *
 * Mirrored in Rust by `timeouts::quality_run_deadline`, which reads the same
 * two constants through `pnpm gen:ipc`; both sides pin the whole table above.
 */
export function qualityRunDeadlineSecs(effort?: string): number {
  return (
    QUALITY_RUN_FIXED_SECS +
    Math.round(STREAM_BASELINE_SECS * QUALITY_RUN_GENERATION_PASSES * effortMultiplier(effort))
  );
}

/**
 * Safety margin (seconds) the RENDERER's own quality-run timeout adds on top of
 * {@link qualityRunDeadlineSecs} — the same relationship, and the same reason,
 * as `stream-promise.ts`'s `OUTER_BOUND_MARGIN_MS` has to `stream_deadline`:
 * the backend must give up FIRST, because it is the side that knows WHY (a
 * `StoppedReason`, a provider error) while the renderer can only say "it timed
 * out".
 */
export const QUALITY_RUN_CLIENT_MARGIN_SECS = 60;

/**
 * The renderer's outer bound (ms) for one quality-depth run — strictly greater
 * than the backend's own deadline at every effort tier, which is what
 * `ai-timeouts.test.ts` pins. Lives here rather than in the renderer so the two
 * halves of that invariant cannot be edited independently.
 */
export function qualityRunClientTimeoutMs(effort?: string): number {
  return (qualityRunDeadlineSecs(effort) + QUALITY_RUN_CLIENT_MARGIN_SECS) * 1000;
}
