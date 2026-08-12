import { describe, expect, it } from 'vitest';

import {
  EFFORT_TIMEOUT_MULTIPLIER,
  MAX_RUN_FIXED_SECS,
  MAX_RUN_GENERATION_PASSES,
  maxRunClientTimeoutMs,
  maxRunDeadlineSecs,
  QUALITY_RUN_CLIENT_MARGIN_SECS,
  QUALITY_RUN_FIXED_SECS,
  QUALITY_RUN_GENERATION_PASSES,
  qualityRunClientTimeoutMs,
  qualityRunDeadlineSecs,
  STREAM_BASELINE_SECS,
} from './ai-timeouts.js';

/**
 * Vendors' ASCENDING tier order — `max` is the TOP tier, above `xhigh`. Written
 * out rather than derived from `Object.keys`, because the whole point of the
 * monotonicity assertions below is to catch a table that was reordered.
 */
const TIERS = [undefined, 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'] as const;

/**
 * The run's INNER per-call bounds, spelled out from constants that are NOT part
 * of `qualityRunDeadlineSecs`' own formula — which is the whole point.
 *
 * The previous version of the "clears the inner bounds" test below recomputed
 * `QUALITY_RUN_FIXED_SECS + STREAM_BASELINE_SECS × QUALITY_RUN_GENERATION_PASSES
 * × multiplier` and compared it against the function built from those exact
 * terms: an identity that cannot fail, and whose claimed mutation ("drop
 * QUALITY_RUN_FIXED_SECS and the bottom tiers fail") was FALSE — both sides move
 * together. These numbers are the Rust-side facts the deadline has to cover, so
 * dropping either shared term now actually fails.
 */
/**
 * These five are HARD LITERALS, deliberately: they are Rust-side facts, and the
 * live guard against them drifting is the RUST twin
 * (`timeouts::quality_run_deadline_clears_the_inner_per_call_bounds`), which
 * reads `Budget::max_repair_attempts` / `MAX_SECTIONS_PER_ROUND` /
 * `OLLAMA_COMPLETION` from the source. This side pins the same arithmetic so a
 * TS-only edit to the deadline formula cannot pass alone.
 */
/**
 * `timeouts::OLLAMA_COMPLETION` — the longest per-call non-streaming bound, and
 * the bound on a whole `send_with_retry` SEQUENCE, not on one attempt (the retry
 * loop applies the caller's timeout and gives a retry only the remainder). Were
 * that budget removed, the real per-call figure would be `MAX_ATTEMPTS × 300`.
 */
const OLLAMA_COMPLETION_SECS = 300;
/** `analyze_job`, `match_evidence`, `strategy`. */
const JSON_STAGES = 3;
/** `Completer::complete_json` allows exactly one re-ask. */
const ROUND_TRIPS_PER_JSON_STAGE = 2;
/** `Budget::max_repair_attempts`. */
const REPAIR_ROUNDS = 2;
/** `pipeline::resume::stages::repair::MAX_SECTIONS_PER_ROUND`. */
const REPAIR_SECTIONS_PER_ROUND = 4;

describe('qualityRunDeadlineSecs', () => {
  it('pins the derived per-tier table', () => {
    // Every value is `QUALITY_RUN_FIXED_SECS + 300 × 1 × multiplier`. Pinned as
    // literals (not recomputed from the constants) so a change to either term
    // has to be re-argued against the derivation in the source doc, and so the
    // Rust twin (`timeouts::quality_run_deadline`) has a table to match.
    expect(qualityRunDeadlineSecs(undefined)).toBe(4_500);
    expect(qualityRunDeadlineSecs('minimal')).toBe(4_500);
    expect(qualityRunDeadlineSecs('low')).toBe(4_500);
    expect(qualityRunDeadlineSecs('medium')).toBe(4_650);
    expect(qualityRunDeadlineSecs('high')).toBe(4_800);
    expect(qualityRunDeadlineSecs('xhigh')).toBe(4_950);
    expect(qualityRunDeadlineSecs('max')).toBe(5_100);
  });

  it('clears the inner per-call bounds it wraps at every tier', () => {
    // The invariant the derivation exists for: the run deadline must exceed the
    // sum of the deadlines the run's own calls are allowed to consume, or it
    // becomes the binding constraint and the actionable per-call error never
    // fires. Mutation checks (both applied, both caught): set
    // QUALITY_RUN_FIXED_SECS back to 1_800 (the pre-fix value that ignored the
    // repair fan-out) and every tier fails; set QUALITY_RUN_GENERATION_PASSES
    // to 0 and every tier fails.
    const flatCalls =
      OLLAMA_COMPLETION_SECS * JSON_STAGES * ROUND_TRIPS_PER_JSON_STAGE +
      OLLAMA_COMPLETION_SECS * REPAIR_ROUNDS * REPAIR_SECTIONS_PER_ROUND;
    for (const effort of TIERS) {
      const multiplier = (effort ? EFFORT_TIMEOUT_MULTIPLIER[effort] : undefined) ?? 1;
      // The draft is the run's only streamed (effort-scaled) call.
      const innerBounds = flatCalls + STREAM_BASELINE_SECS * multiplier;
      expect(qualityRunDeadlineSecs(effort)).toBeGreaterThanOrEqual(innerBounds);
    }
  });

  it('accounts for the repair fan-out in a term that does not scale with effort', () => {
    // The second half of the AH2 fix, pinned separately from the sum above:
    // the 8 repair calls are bounded by a FLAT constant, so they must sit in
    // `QUALITY_RUN_FIXED_SECS`. Mutation check: move them back into the scaled
    // term (FIXED 1_800 + PASSES 9) and the fixed-term assertion fails — while
    // the sum above still passes at the bottom tier, which is exactly why this
    // needs its own guard.
    expect(QUALITY_RUN_FIXED_SECS).toBeGreaterThanOrEqual(
      OLLAMA_COMPLETION_SECS * JSON_STAGES * ROUND_TRIPS_PER_JSON_STAGE +
        OLLAMA_COMPLETION_SECS * REPAIR_ROUNDS * REPAIR_SECTIONS_PER_ROUND
    );
    expect(QUALITY_RUN_GENERATION_PASSES).toBe(1);
  });

  it('is monotonically nondecreasing across the ascending tier order', () => {
    let previous = 0;
    for (const effort of TIERS) {
      const deadline = qualityRunDeadlineSecs(effort);
      expect(deadline).toBeGreaterThanOrEqual(previous);
      previous = deadline;
    }
    // Not vacuous: the top tier must actually exceed the baseline.
    expect(qualityRunDeadlineSecs('max')).toBeGreaterThan(qualityRunDeadlineSecs(undefined));
  });

  it('falls back to the baseline for an unrecognized effort string', () => {
    expect(qualityRunDeadlineSecs('ultra-mega-think')).toBe(qualityRunDeadlineSecs(undefined));
    expect(qualityRunDeadlineSecs('')).toBe(qualityRunDeadlineSecs(undefined));
  });
});

describe('qualityRunClientTimeoutMs', () => {
  it('strictly exceeds the backend deadline at every effort tier', () => {
    // THE lock: the backend must give up first, because it is the side that
    // knows WHY. Mutation check: set QUALITY_RUN_CLIENT_MARGIN_SECS to 0 and
    // every assertion here fails.
    for (const effort of TIERS) {
      expect(qualityRunClientTimeoutMs(effort)).toBeGreaterThan(
        qualityRunDeadlineSecs(effort) * 1000
      );
    }
    expect(QUALITY_RUN_CLIENT_MARGIN_SECS).toBeGreaterThan(0);
  });

  it('covers the longest call that can still be in flight when the deadline expires', () => {
    // "Strictly greater" is not enough on its own. The backend checks its
    // deadline BETWEEN provider calls, so it reports `run_timeout` only once
    // the call that was running returns — up to one whole per-call bound after
    // the deadline. A margin smaller than that means the renderer times out
    // first on every run that actually times out, which is the inversion the
    // test above only LOOKS like it prevents.
    //
    // Mutation checks: margin back to 60 ⇒ fails; raise the top multiplier to
    // 4.0 without raising the margin ⇒ fails.
    const topMultiplier = Math.max(...Object.values(EFFORT_TIMEOUT_MULTIPLIER));
    const longestInFlightCall = Math.max(
      OLLAMA_COMPLETION_SECS, // any flat call: a JSON stage, a repair splice
      STREAM_BASELINE_SECS * topMultiplier // the draft at the top tier
    );
    expect(QUALITY_RUN_CLIENT_MARGIN_SECS).toBeGreaterThan(longestInFlightCall);
  });

  it('is monotonically nondecreasing across the ascending tier order', () => {
    let previous = 0;
    for (const effort of TIERS) {
      const timeout = qualityRunClientTimeoutMs(effort);
      expect(timeout).toBeGreaterThanOrEqual(previous);
      previous = timeout;
    }
  });
});

// ── Max depth ───────────────────────────────────────────────────────────────
//
// Same shape, same rules, different fan-out. These five are Rust-side facts of
// the MAX pipeline, spelled out as literals for the same reason as above: they
// are NOT terms of the deadline's own formula, so dropping one actually fails.
/** `Budget::RESUME_MAX.max_sections` — one provider call each. */
const MAX_SECTIONS = 12;
/** The `llm_judge` stage: one `complete_json` over the finished document. */
const JUDGE_CALLS = 1;

describe('maxRunDeadlineSecs', () => {
  it('pins the derived per-tier table', () => {
    // `MAX_RUN_FIXED_SECS + 300 × 1 × multiplier`, matching Rust's
    // `timeouts::max_run_deadline` exactly — this side is a HAND MIRROR of that
    // constant (gen:ipc does not emit it), so the literals are the whole guard.
    expect(maxRunDeadlineSecs(undefined)).toBe(7_500);
    expect(maxRunDeadlineSecs('minimal')).toBe(7_500);
    expect(maxRunDeadlineSecs('low')).toBe(7_500);
    expect(maxRunDeadlineSecs('medium')).toBe(7_650);
    expect(maxRunDeadlineSecs('high')).toBe(7_800);
    expect(maxRunDeadlineSecs('xhigh')).toBe(7_950);
    expect(maxRunDeadlineSecs('max')).toBe(8_100);
  });

  it('agrees with `Budget::RESUME_MAX.run_timeout` at the bottom tier', () => {
    // `resume::run_deadline` takes the LARGER of the budget floor and the
    // effort-scaled value, so a disagreement means one of the two has stopped
    // describing what runs. 125 minutes is the budget constant.
    expect(maxRunDeadlineSecs(undefined)).toBe(125 * 60);
  });

  it('clears every call a max run PLANS to make, at every tier', () => {
    // Max depth streams nothing, so all 24 calls carry the flat per-call bound,
    // the judge's included — and the deadline must EXCEED that, not merely
    // equal it: the backend notices its deadline between calls, so a bound that
    // is exactly the fan-out leaves the last call no room to return.
    //
    // Mutation checks, all applied and reverted: MAX_RUN_FIXED_SECS
    // 7_200 → 6_600 ⇒ fails; 7_200 → 6_900 (the pre-judge value, which counted
    // 23 calls) ⇒ fails; MAX_RUN_GENERATION_PASSES 1 → 0 ⇒ fails.
    //
    // The 6_900 case is why this is `toBeGreaterThan` and not
    // `toBeGreaterThanOrEqual`: it puts the bottom tier at exactly 7_200 s,
    // dead level with the fan-out it is supposed to wrap, and a `>=` here would
    // have called that fine.
    const planned =
      JSON_STAGES + MAX_SECTIONS + JUDGE_CALLS + REPAIR_ROUNDS * REPAIR_SECTIONS_PER_ROUND;
    const innerBounds = OLLAMA_COMPLETION_SECS * planned;
    for (const effort of TIERS) {
      expect(maxRunDeadlineSecs(effort)).toBeGreaterThan(innerBounds);
    }
  });

  it('is never shorter than the quality deadline at the same tier', () => {
    // Max depth is strictly more work than quality depth, so the inversion the
    // effort table already suffered once (a bigger tier with a smaller bound)
    // must not reappear between the two DEPTHS.
    for (const effort of TIERS) {
      expect(maxRunDeadlineSecs(effort)).toBeGreaterThanOrEqual(qualityRunDeadlineSecs(effort));
    }
  });

  it('is monotonically nondecreasing across the ascending tier order', () => {
    let previous = 0;
    for (const effort of TIERS) {
      const deadline = maxRunDeadlineSecs(effort);
      expect(deadline).toBeGreaterThanOrEqual(previous);
      previous = deadline;
    }
    expect(maxRunDeadlineSecs('max')).toBeGreaterThan(maxRunDeadlineSecs(undefined));
  });

  it('falls back to the baseline for an unrecognized effort string', () => {
    expect(maxRunDeadlineSecs('ultra-mega-think')).toBe(maxRunDeadlineSecs(undefined));
    expect(maxRunDeadlineSecs('')).toBe(maxRunDeadlineSecs(undefined));
  });
});

describe('maxRunClientTimeoutMs', () => {
  it('strictly exceeds the backend deadline at every effort tier', () => {
    // THE lock, max half: the backend must give up first, because it is the
    // side that knows WHY. Share the quality margin or size a new one — either
    // way it has to stay above this.
    for (const effort of TIERS) {
      expect(maxRunClientTimeoutMs(effort)).toBeGreaterThan(maxRunDeadlineSecs(effort) * 1000);
    }
  });

  it('covers the longest call that can still be in flight at max depth', () => {
    // Max depth streams NOTHING, so its longest in-flight call is the flat
    // per-call bound — strictly easier than quality's top-tier draft, which is
    // why one margin honestly covers both.
    expect(QUALITY_RUN_CLIENT_MARGIN_SECS).toBeGreaterThan(OLLAMA_COMPLETION_SECS);
  });

  it('is monotonically nondecreasing across the ascending tier order', () => {
    let previous = 0;
    for (const effort of TIERS) {
      const timeout = maxRunClientTimeoutMs(effort);
      expect(timeout).toBeGreaterThanOrEqual(previous);
      previous = timeout;
    }
  });

  it('never gives a max run a shorter client bound than a quality one', () => {
    for (const effort of TIERS) {
      expect(maxRunClientTimeoutMs(effort)).toBeGreaterThanOrEqual(
        qualityRunClientTimeoutMs(effort)
      );
    }
    // Not vacuous — the two formulas share a margin, so this is really the
    // deadline relation, and it must hold after any re-margining too.
    expect(MAX_RUN_FIXED_SECS).toBeGreaterThan(QUALITY_RUN_FIXED_SECS);
    expect(MAX_RUN_GENERATION_PASSES).toBe(1);
  });
});
