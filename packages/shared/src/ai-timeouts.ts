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
 * seconds — every call the run makes whose per-call bound is a FLAT constant.
 *
 * Derived, not guessed, and derived from the fan-out that actually runs:
 *
 * | term                       | calls           | per-call bound       | total  |
 * | -------------------------- | --------------- | -------------------- | ------ |
 * | 3 JSON stages, ≤1 re-ask   | 3 × 2 = 6       | `OLLAMA_COMPLETION`  | 1800 s |
 * | repair, ≤2 rounds × ≤4 sec | 2 × 4 = 8       | `OLLAMA_COMPLETION`  | 2400 s |
 * | humanize, ≤2 documents     | 2               | `OLLAMA_COMPLETION`  |  600 s |
 * | **fixed total**            |                 |                      | 4800 s |
 *
 * `analyze_job`/`match_evidence`/`strategy` each run through
 * `Completer::complete_json`, which is allowed exactly ONE re-ask. The repair
 * stage regenerates up to `repair::MAX_SECTIONS_PER_ROUND` (4) sections per
 * round for up to `Budget::max_repair_attempts` (2) rounds, each through
 * `Completer::complete`. The `humanize` stage makes at most one `complete` call
 * PER FLAGGED DOCUMENT (résumé + letter), so at most 2. **Every one of those 16
 * round-trips is bounded by the longest per-call HTTP timeout this app ships —
 * `timeouts::OLLAMA_COMPLETION`, 300 s** (the local daemon's non-streaming
 * bound), and NONE of them scales with reasoning effort: `complete_with_usage`'s
 * timeouts (`COMPLETION` / `OLLAMA_COMPLETION`) are flat constants. Only the
 * STREAM deadline is effort-scaled, and `draft`/`cover_letter` are the run's
 * only streamed calls (see {@link QUALITY_RUN_GENERATION_PASSES}).
 *
 * **"300 s per call" is a statement about the RETRY LOOP, not just the
 * `.timeout()`.** `commands::ai_provider::retry::send_with_retry` re-sends a
 * transient failure up to `MAX_ATTEMPTS` (3) times, so it has to bound the whole
 * sequence by the caller's timeout — which it does, applying the timeout itself
 * and giving a retry only what is left of it. It did NOT when this term was first
 * written: each attempt rebuilt its own full `.timeout()`, so one "300 s" call
 * really cost up to 3 × 300 s + backoff (901 s) and these 14 of them up to
 * ~12 600 s against an advertised 4 500 s. If that budget is ever removed, this
 * whole derivation is wrong by a factor of `MAX_ATTEMPTS` — which is why the
 * retry loop's own budget test is a load-bearing part of this number.
 *
 * The rule this exists to satisfy is the same one
 * `research_deadline_exceeds_the_inner_search_bounds_it_wraps` states for
 * research: **an outer bound that does not clear the inner bounds it wraps
 * becomes the binding constraint**, and the actionable inner error never gets
 * to fire. The repair half used to be counted as one effort-scaled
 * draft-equivalent PER ROUND (600 s at the baseline) rather than as its real 8
 * flat-bounded calls (2400 s), so the advertised deadline was ~1800 s short of
 * the fan-out it wraps — and, because the deadline was only checked at a stage
 * boundary and repair is the LAST stage, nothing else bounded it either. Both
 * halves were fixed together: the repair loop now checks the deadline between
 * rounds AND between per-section calls, and this term covers the fan-out.
 *
 * **PR-2 addition: `humanize`.** It runs LAST and makes at most one
 * `Completer::complete` call per FLAGGED document (résumé, letter), so ≤2 more
 * flat `OLLAMA_COMPLETION`-bounded calls — 600 s, taking the fixed term from
 * 4 200 s to 4 800 s. Zero flagged documents costs nothing (the deterministic
 * `voice.*` scan that gates it is not a provider call), so this is the WORST
 * case, exactly like every other term above.
 */
export const QUALITY_RUN_FIXED_SECS = 4_800;

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
 * `fixed + baseline × passes × multiplier(effort)`, i.e. the sum of the inner
 * per-call bounds the run can legitimately consume at that effort:
 *
 * | effort           | m   | flat calls | 2 generation passes | deadline         |
 * | ---------------- | --- | ---------- | -------------------- | ---------------- |
 * | none/minimal/low | 1.0 | 4800 s     | 600 s                | 5400 s (90 min)  |
 * | medium           | 1.5 | 4800 s     | 900 s                | 5700 s (95 min)  |
 * | high             | 2.0 | 4800 s     | 1200 s                | 6000 s (100 min) |
 * | xhigh            | 2.5 | 4800 s     | 1500 s                | 6300 s (105 min) |
 * | max              | 3.0 | 4800 s     | 1800 s                | 6600 s (110 min) |
 *
 * Deliberately NOT `baseline × multiplier`: all but one of the run's calls do
 * not scale with effort at all (see {@link QUALITY_RUN_FIXED_SECS}), so a
 * single multiplicative constant either under-provisions the bottom tier —
 * killing legitimate runs — or wildly over-provisions the top one.
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
 *
 * **Sized against the OVERSHOOT, not picked as a round number.** The backend
 * observes its deadline BETWEEN provider calls (`StageHooks::before`, the repair
 * loop's per-round and per-section checks, and the guard in front of a JSON
 * stage's re-ask) — never mid-call, because cancelling an in-flight completion
 * would throw away work the run has already paid for. So a run whose deadline
 * expires one millisecond into a call reports `run_timeout` only when that call
 * returns, and the renderer must still be waiting then:
 *
 * | in-flight call when the deadline expires | bound                        |
 * | ---------------------------------------- | ---------------------------- |
 * | any flat call (JSON stage, repair splice) | `OLLAMA_COMPLETION` 300 s   |
 * | the draft stream, top tier                | 300 × 3.0 = 900 s           |
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

/**
 * The EFFORT-INVARIANT half of one MAX-depth run's deadline, in seconds.
 *
 * 24 calls at the flat `timeouts::OLLAMA_COMPLETION` (300 s) bound: 3 JSON
 * stages (analyze, evidence, strategy) + `Budget::RESUME_MAX.max_sections` (12)
 * section calls + the `llm_judge` stage's single call + `max_repair_attempts`
 * (2) × `MAX_SECTIONS_PER_ROUND` (4) repair rewrites. Unlike
 * {@link QUALITY_RUN_FIXED_SECS} this covers the WHOLE planned run rather than
 * only its effort-invariant part, because **max depth streams nothing** — every
 * one of its calls goes through `Completer`, whose timeouts are flat constants.
 *
 * **The judge is the 24th call and it is counted HERE**, not left to the scaled
 * term. An earlier version of this constant (6 900 s) counted 23 and let the
 * baseline tier's scaled 300 s cover the judge, which made the bottom tier
 * exactly equal to its own fan-out — no slack at all, and a deadline that binds
 * before the last call can return is the failure this whole derivation exists
 * to prevent.
 *
 * The one allowed re-ask per JSON call is deliberately NOT counted, on both
 * sides — the same departure from the quality derivation the Rust twin records:
 * a re-ask is a parse-failure path guarded by the same clock, and a max run
 * stopped mid-fan-out KEEPS the sections it already assembled, so buying the
 * worst case would only make the deadline unreachable.
 *
 * **Hand-mirrored from Rust, not generated.** `timeouts::MAX_RUN_FIXED_SECS`
 * is a Rust constant that `pnpm gen:ipc` does not emit (its doc says so: the
 * renderer had no max deadline when it was written). The per-tier table below
 * is pinned as literals on this side and the Rust side pins the same
 * relationships, so the pair cannot drift silently — but the next change to
 * either constant should move it into `gen-ipc-rust.ts` alongside its quality
 * sibling and delete this copy.
 */
export const MAX_RUN_FIXED_SECS = 7_200;

/**
 * Effort-SCALED whole-document passes one max run may make: one.
 *
 * The fan-out writes the document once and streams nothing, but the run's real
 * duration still scales with the reasoning budget — this term is what keeps a
 * high-effort run from being stopped by a deadline sized for a fast one. It
 * buys no particular call: {@link MAX_RUN_FIXED_SECS} already covers all 24 of
 * them, so this is headroom on top, which is what makes the bottom tier's
 * deadline exceed its fan-out rather than merely equal it.
 */
export const MAX_RUN_GENERATION_PASSES = 1;

/**
 * Deadline (seconds) for ONE WHOLE max-depth run — the renderer's mirror of the
 * backend's `timeouts::max_run_deadline`, and what makes
 * `StoppedReason::RunTimeout` reachable at this depth.
 *
 * | effort           | m   | fixed  | scaled | deadline         |
 * | ---------------- | --- | ------ | ------ | ---------------- |
 * | none/minimal/low | 1.0 | 7200 s | 300 s  | 7500 s (125 min) |
 * | medium           | 1.5 | 7200 s | 450 s  | 7650 s (128 min) |
 * | high             | 2.0 | 7200 s | 600 s  | 7800 s (130 min) |
 * | xhigh            | 2.5 | 7200 s | 750 s  | 7950 s (133 min) |
 * | max              | 3.0 | 7200 s | 900 s  | 8100 s (135 min) |
 *
 * The bottom tier equals `Budget::RESUME_MAX.run_timeout` (125 min) on purpose:
 * `resume::run_deadline` takes the LARGER of the budget floor and this, so a
 * disagreement would mean one of the two stops describing what runs.
 *
 * Never shorter than {@link qualityRunDeadlineSecs} at the same tier — max
 * depth is strictly more work — which is a lock test, not a coincidence.
 */
export function maxRunDeadlineSecs(effort?: string): number {
  return (
    MAX_RUN_FIXED_SECS +
    Math.round(STREAM_BASELINE_SECS * MAX_RUN_GENERATION_PASSES * effortMultiplier(effort))
  );
}

/**
 * The renderer's outer bound (ms) for one max-depth run — strictly greater than
 * {@link maxRunDeadlineSecs} at every tier, for the reason
 * {@link QUALITY_RUN_CLIENT_MARGIN_SECS} exists: the backend must give up FIRST,
 * because it is the side that knows WHY.
 *
 * Shares the quality margin rather than sizing a second one, and is safe doing
 * so: the margin has to cover the longest call that can still be IN FLIGHT when
 * the deadline expires (the backend checks it BETWEEN calls), and max depth's
 * longest is the 300 s flat `OLLAMA_COMPLETION` — it streams nothing at all,
 * where quality's draft can be in flight for 900 s at the top tier. A margin
 * that clears the harder case clears this one, and over-waiting is the safe
 * direction for a bound the client can only trip early.
 *
 * **RESERVED, exactly like its quality sibling** — `resumePipeline.run` returns
 * on admission and completion arrives through the record poll, so there is no
 * long-lived request to bound. It exists so a future caller does not invent its
 * own margin, which is precisely how the renderer once ended up giving up
 * before the backend did.
 */
export function maxRunClientTimeoutMs(effort?: string): number {
  return (maxRunDeadlineSecs(effort) + QUALITY_RUN_CLIENT_MARGIN_SECS) * 1000;
}
