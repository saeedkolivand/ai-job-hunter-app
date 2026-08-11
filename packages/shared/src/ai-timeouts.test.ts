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

describe('qualityRunDeadlineSecs', () => {
  it('pins the derived per-tier table', () => {
    // Every value is `QUALITY_RUN_FIXED_SECS + 300 × 3 × multiplier`. Pinned as
    // literals (not recomputed from the constants) so a change to either term
    // has to be re-argued against the derivation in the source doc, and so the
    // Rust twin (`timeouts::quality_run_deadline`) has a table to match.
    expect(qualityRunDeadlineSecs(undefined)).toBe(2_700);
    expect(qualityRunDeadlineSecs('minimal')).toBe(2_700);
    expect(qualityRunDeadlineSecs('low')).toBe(2_700);
    expect(qualityRunDeadlineSecs('medium')).toBe(3_150);
    expect(qualityRunDeadlineSecs('high')).toBe(3_600);
    expect(qualityRunDeadlineSecs('xhigh')).toBe(4_050);
    expect(qualityRunDeadlineSecs('max')).toBe(4_500);
  });

  it('clears the inner per-call bounds it wraps at every tier', () => {
    // The invariant the derivation exists for: the run deadline must exceed the
    // sum of the deadlines the run's own calls are allowed to consume, or it
    // becomes the binding constraint and the actionable per-call error never
    // fires. Mutation check: drop `QUALITY_RUN_FIXED_SECS` to the old flat
    // 30-minute budget (1_800 → 900) and the bottom tiers fail.
    for (const effort of TIERS) {
      const multiplier = (effort ? EFFORT_TIMEOUT_MULTIPLIER[effort] : undefined) ?? 1;
      const innerBounds =
        QUALITY_RUN_FIXED_SECS + STREAM_BASELINE_SECS * QUALITY_RUN_GENERATION_PASSES * multiplier;
      expect(qualityRunDeadlineSecs(effort)).toBeGreaterThanOrEqual(innerBounds);
    }
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

  it('is monotonically nondecreasing across the ascending tier order', () => {
    let previous = 0;
    for (const effort of TIERS) {
      const timeout = qualityRunClientTimeoutMs(effort);
      expect(timeout).toBeGreaterThanOrEqual(previous);
      previous = timeout;
    }
  });
});
