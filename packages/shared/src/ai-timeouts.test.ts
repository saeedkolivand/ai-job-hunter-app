import { describe, expect, it } from 'vitest';

import {
  EFFORT_TIMEOUT_MULTIPLIER,
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
/** `humanize` — at most one flat `complete` call per flagged document. */
const HUMANIZE_MAX_CALLS = 2;

describe('qualityRunDeadlineSecs', () => {
  it('pins the derived per-tier table', () => {
    // Every value is `QUALITY_RUN_FIXED_SECS + 300 × 2 × multiplier`. Pinned as
    // literals (not recomputed from the constants) so a change to either term
    // has to be re-argued against the derivation in the source doc, and so the
    // Rust twin (`timeouts::quality_run_deadline`) has a table to match.
    expect(qualityRunDeadlineSecs(undefined)).toBe(5_400);
    expect(qualityRunDeadlineSecs('minimal')).toBe(5_400);
    expect(qualityRunDeadlineSecs('low')).toBe(5_400);
    expect(qualityRunDeadlineSecs('medium')).toBe(5_700);
    expect(qualityRunDeadlineSecs('high')).toBe(6_000);
    expect(qualityRunDeadlineSecs('xhigh')).toBe(6_300);
    expect(qualityRunDeadlineSecs('max')).toBe(6_600);
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
      OLLAMA_COMPLETION_SECS * REPAIR_ROUNDS * REPAIR_SECTIONS_PER_ROUND +
      OLLAMA_COMPLETION_SECS * HUMANIZE_MAX_CALLS;
    for (const effort of TIERS) {
      const multiplier = (effort ? EFFORT_TIMEOUT_MULTIPLIER[effort] : undefined) ?? 1;
      // The draft and the letter are the run's only streamed (effort-scaled)
      // calls — see QUALITY_RUN_GENERATION_PASSES.
      const innerBounds =
        flatCalls + STREAM_BASELINE_SECS * QUALITY_RUN_GENERATION_PASSES * multiplier;
      expect(qualityRunDeadlineSecs(effort)).toBeGreaterThanOrEqual(innerBounds);
    }
  });

  it('accounts for the repair fan-out and the humanize allowance in a term that does not scale with effort', () => {
    // The second half of the AH2 fix, pinned separately from the sum above:
    // the 8 repair calls plus humanize's ≤2 are bounded by a FLAT constant, so
    // they must sit in `QUALITY_RUN_FIXED_SECS`. Mutation check: move them back
    // into the scaled term and the fixed-term assertion fails — while the sum
    // above still passes at the bottom tier, which is exactly why this needs
    // its own guard.
    expect(QUALITY_RUN_FIXED_SECS).toBeGreaterThanOrEqual(
      OLLAMA_COMPLETION_SECS * JSON_STAGES * ROUND_TRIPS_PER_JSON_STAGE +
        OLLAMA_COMPLETION_SECS * REPAIR_ROUNDS * REPAIR_SECTIONS_PER_ROUND +
        OLLAMA_COMPLETION_SECS * HUMANIZE_MAX_CALLS
    );
    expect(QUALITY_RUN_GENERATION_PASSES).toBe(2);
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
