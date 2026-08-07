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
