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
 * Baseline NON-streaming completion deadline, in seconds, for no/low/
 * unrecognized effort — the local-Ollama analogue of {@link STREAM_BASELINE_SECS}.
 * Mirrored in Rust as `timeouts::OLLAMA_COMPLETION_BASELINE`, scaled the exact
 * same way {@link STREAM_BASELINE_SECS} is (`ollamaCompletionDeadlineSecs`
 * below / `timeouts::ollama_completion_deadline`).
 *
 * A SEPARATE constant from `STREAM_BASELINE_SECS`, not a reused one, even
 * though the two happen to share a value today — they bound different
 * operations (a whole streamed generation vs. one non-streaming `/api/chat`
 * call), per this file's own "same duration + different operation gets its
 * own constant" rule (mirrored from `timeouts.rs`'s module doc), so they can
 * drift independently later.
 *
 * **Why this exists at all — the incident this fixes.** Before it, EVERY
 * non-streaming call (`analyze_job`/`match_evidence`/`strategy`, each one
 * `Completer::complete_json`'s only round-trip) was pinned to this same 300 s
 * regardless of the run's chosen effort — a local model running at a HIGHER
 * effort got no more time on these three calls than the baseline did, so
 * raising effort could not have saved a run that legitimately needed longer
 * on a slow local model. `chat_stream`'s deadline (`STREAM_BASELINE_SECS`)
 * scaled; this one didn't, silently.
 */
export const OLLAMA_COMPLETION_BASELINE_SECS = 300;

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
 * The actual per-request deadline for a non-streaming `/api/chat` call:
 * {@link OLLAMA_COMPLETION_BASELINE_SECS} scaled by {@link effortMultiplier} —
 * exactly how the streaming deadline scales {@link STREAM_BASELINE_SECS}.
 * Rust twin: `timeouts::ollama_completion_deadline`.
 */
export function ollamaCompletionDeadlineSecs(effort?: string): number {
  return Math.round(OLLAMA_COMPLETION_BASELINE_SECS * effortMultiplier(effort));
}

/**
 * How many non-streaming round-trips `analyze_job`/`match_evidence`/
 * `strategy` make in the WORST case: 3 stages × (1 call + 1 allowed re-ask).
 * Each goes through `Completer::complete_json`, which re-asks exactly once on
 * a parse failure.
 *
 * Named so {@link qualityRunDeadlineSecs}'s formula reads as the derivation it
 * is rather than a bare `6`, and so the Rust-side lock test
 * (`quality_run_deadline_clears_the_inner_per_call_bounds`) has one shared
 * name to read instead of re-deriving `3 × 2` from a comment.
 */
export const QUALITY_RUN_JSON_STAGE_CALLS = 6;

/**
 * The part of one quality-depth pipeline run's deadline that does NOT scale
 * with effort, in seconds: the repair fan-out plus `humanize`'s allowance —
 * both bounded by the FLAT {@link OLLAMA_COMPLETION_BASELINE_SECS} regardless
 * of the run's effort, because neither call site currently has an effort to
 * scale by (`Completer::complete`, unlike `complete_json`, takes no
 * `AiGenerateRequest` and so carries no `effort` field to read).
 *
 * Derived, not guessed, from the fan-out that actually runs:
 *
 * | term                       | calls     | per-call bound                     | total  |
 * | -------------------------- | --------- | ----------------------------------- | ------ |
 * | repair, ≤2 rounds × ≤4 sec | 2 × 4 = 8 | `OLLAMA_COMPLETION_BASELINE_SECS`  | 2400 s |
 * | humanize, ≤2 documents     | 2         | `OLLAMA_COMPLETION_BASELINE_SECS`  |  600 s |
 * | **flat total**             |           |                                      | 3000 s |
 *
 * The repair stage regenerates up to `repair::MAX_SECTIONS_PER_ROUND` (4)
 * sections per round for up to `Budget::max_repair_attempts` (2) rounds, each
 * through `Completer::complete`. The `humanize` stage makes at most one
 * `complete` call PER FLAGGED DOCUMENT (résumé + letter), so at most 2.
 *
 * **`analyze_job`/`match_evidence`/`strategy` used to live in this same flat
 * term too** (making it 4 800 s — {@link QUALITY_RUN_JSON_STAGE_CALLS} (6) +
 * 8 + 2 = 16 calls, all at a flat 300 s). That was the bug this split exists
 * to fix: those three stages run FIRST, on the SAME local model as
 * everything else, and a flat per-call bound meant no effort setting could
 * give a slow local model more time on them — even though the exact same
 * model legitimately got more time on the STREAMED `draft`/`cover_letter`
 * calls right after. They now scale in {@link qualityRunDeadlineSecs}'s own
 * formula via {@link ollamaCompletionDeadlineSecs} ×
 * {@link QUALITY_RUN_JSON_STAGE_CALLS} instead of living in this flat term.
 *
 * **"300 s per call" is a statement about the RETRY LOOP, not just the
 * `.timeout()`.** `commands::ai_provider::retry::send_with_retry` re-sends a
 * transient failure up to `MAX_ATTEMPTS` (3) times, so it has to bound the whole
 * sequence by the caller's timeout — which it does, applying the timeout itself
 * and giving a retry only what is left of it. If that budget were ever
 * removed, this whole derivation would be wrong by a factor of `MAX_ATTEMPTS`
 * — which is why the retry loop's own budget test is a load-bearing part of
 * this number.
 *
 * The rule this exists to satisfy is the same one
 * `research_deadline_exceeds_the_inner_search_bounds_it_wraps` states for
 * research: **an outer bound that does not clear the inner bounds it wraps
 * becomes the binding constraint**, and the actionable inner error never gets
 * to fire.
 */
export const QUALITY_RUN_FIXED_SECS = 3_000;

/**
 * How many EFFORT-SCALED (streamed) whole-document passes one quality-depth run
 * may make: two — the résumé draft, and the cover letter when
 * `includeCoverLetter` is set.
 *
 * Both are the run's only `chat_stream` calls, and therefore the only calls
 * bounded by {@link STREAM_BASELINE_SECS} × the effort multiplier
 * (`stream_deadline`). The repair rounds and `humanize` are NOT here: they go
 * through `Completer::complete`, whose bound is flat, so they belong to
 * {@link QUALITY_RUN_FIXED_SECS} — counting them here would scale calls that do
 * not scale, wildly over-provisioning the top tier for the same reason
 * `baseline × multiplier` under-provisions the bottom one.
 *
 * **Charged for every run, whether or not a letter is requested** — the SAME
 * choice {@link QUALITY_RUN_FIXED_SECS} already makes for `humanize`'s worst
 * case: the deadline is a backstop sized against what a run COULD spend, not
 * against what THIS run will.
 */
export const QUALITY_RUN_GENERATION_PASSES = 2;

/**
 * Deadline (seconds) for ONE WHOLE quality-depth résumé pipeline run — the
 * backend's `StoppedReason::RunTimeout` trigger.
 *
 * `flat + jsonStages × ollamaCompletionDeadline(effort) + baseline × passes ×
 * multiplier(effort)`, i.e. the sum of the inner per-call bounds the run can
 * legitimately consume at that effort — TWO of the three terms now scale,
 * since the non-streaming JSON-stage bound scales the same way the streamed
 * one does (see {@link ollamaCompletionDeadlineSecs}):
 *
 * | effort            | m   | flat (repair+humanize) | 6 JSON-stage calls | 2 generation passes | deadline          |
 * | ----------------- | --- | ------------------------ | -------------------- | -------------------- | ----------------- |
 * | none/minimal/low  | 1.0 | 3000 s                   | 1800 s                | 600 s                 | 5400 s (90 min)   |
 * | medium            | 1.5 | 3000 s                   | 2700 s                | 900 s                 | 6600 s (110 min)  |
 * | high              | 2.0 | 3000 s                   | 3600 s                | 1200 s                | 7800 s (130 min)  |
 * | xhigh             | 2.5 | 3000 s                   | 4500 s                | 1500 s                | 9000 s (150 min)  |
 * | max               | 3.0 | 3000 s                   | 5400 s                | 1800 s                | 10200 s (170 min) |
 *
 * The bottom tier is UNCHANGED (still 5400 s / 90 min — `multiplier` is 1.0
 * there regardless of which term it applies to), which is what keeps this the
 * same floor `Budget::RESUME_QUALITY.run_timeout` pins. Every tier above it
 * moved: raising the per-call ceiling on 6 of the run's 16 calls without
 * re-deriving this deadline would have made THIS the new silent cap on the
 * exact stages it was just raised to unblock.
 *
 * Still deliberately not a single `baseline × multiplier`: the repair fan-out
 * and `humanize` (see {@link QUALITY_RUN_FIXED_SECS}) stay flat-bounded — a
 * single multiplicative constant across all three terms would either
 * under-provision the bottom tier or wildly over-provision the top one.
 *
 * This is a BACKSTOP for a run that never trips a per-step timeout but crawls
 * forever, not a target: the realistic clean quality run is +30–90 s over the
 * one-shot path. `Budget::step_timeout` is what catches a single hung call.
 *
 * Mirrored in Rust by `timeouts::quality_run_deadline`, which reads the same
 * constants through `pnpm gen:ipc`; both sides pin the whole table above.
 */
export function qualityRunDeadlineSecs(effort?: string): number {
  return (
    QUALITY_RUN_FIXED_SECS +
    QUALITY_RUN_JSON_STAGE_CALLS * ollamaCompletionDeadlineSecs(effort) +
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
 *
 * **Sized against the OVERSHOOT, not picked as a round number.** The backend
 * observes its deadline BETWEEN provider calls (`StageHooks::before`, the repair
 * loop's per-round and per-section checks, and the guard in front of a JSON
 * stage's re-ask) — never mid-call, because cancelling an in-flight completion
 * would throw away work the run has already paid for. So a run whose deadline
 * expires one millisecond into a call reports `run_timeout` only when that call
 * returns, and the renderer must still be waiting then:
 *
 * | in-flight call when the deadline expires    | bound                       |
 * | -------------------------------------------- | --------------------------- |
 * | repair splice / humanize rewrite, always flat | `OLLAMA_COMPLETION_BASELINE_SECS` 300 s |
 * | a JSON stage's call, top tier                 | 300 × 3.0 = 900 s (`ollamaCompletionDeadlineSecs`) |
 * | the draft stream, top tier                    | 300 × 3.0 = 900 s (`stream_deadline`)   |
 *
 * A JSON-stage call now reaches the same top-tier bound the draft stream
 * already did (both baselines are 300 s scaled by the same multiplier table),
 * so the worst case is still 900 s, not a new, larger number — the margin
 * below did not need to grow when the JSON-stage bound started scaling.
 *
 * 900 + 60 s of slack for validation, persistence and the IPC hop. At 60 s (the
 * previous value) the renderer's own timeout fired first on every run that
 * actually timed out — precisely the inversion this constant exists to prevent.
 *
 * Only ONE call can be in flight at a time, which is what keeps this a single
 * per-call bound rather than a multiple: the run is sequential, the retry loop
 * bounds its whole sequence by that call's timeout (see
 * {@link QUALITY_RUN_FIXED_SECS}), and a JSON stage's re-ask is refused once the
 * deadline has passed rather than adding a second call after it.
 *
 * Flat rather than effort-scaled on purpose: over-waiting is the SAFE direction
 * for this bound (the renderer only ever gives up early, never late), and a
 * second effort-dependent formula on the client side buys nothing.
 */
export const QUALITY_RUN_CLIENT_MARGIN_SECS = 960;

/**
 * The renderer's outer bound (ms) for one quality-depth run — strictly greater
 * than the backend's own deadline at every effort tier, which is what
 * `ai-timeouts.test.ts` pins. Lives here rather than in the renderer so the two
 * halves of that invariant cannot be edited independently.
 *
 * **RESERVED — no production consumer yet, deliberately.** The staged run is
 * started by `resumePipeline.run`, which returns as soon as the run is ADMITTED
 * and reports completion through the record poll, so there is no long-lived
 * request for a transport timeout to bound. Kept (and kept tested) because the
 * value that WOULD be needed is the one thing a future caller must not
 * re-derive: it has to stay above `qualityRunDeadlineSecs` at every tier, and a
 * caller inventing its own margin is exactly how the renderer ended up giving
 * up before the backend did. Wire it only alongside a call that can actually
 * hang.
 */
export function qualityRunClientTimeoutMs(effort?: string): number {
  return (qualityRunDeadlineSecs(effort) + QUALITY_RUN_CLIENT_MARGIN_SECS) * 1000;
}
