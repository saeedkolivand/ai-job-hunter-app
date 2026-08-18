import { describe, expect, it } from 'vitest';

import { type ConstraintCheck, type ConstraintStatus, isKnockOut } from './index.js';

const check = (status: ConstraintStatus): ConstraintCheck => ({ id: 'location', status });

describe('isKnockOut', () => {
  it('is true for exactly one status', () => {
    expect(isKnockOut(check('notMet'))).toBe(true);
    expect(isKnockOut(check('met'))).toBe(false);
    expect(isKnockOut(check('unknown'))).toBe(false);
    expect(isKnockOut(check('noPreference'))).toBe(false);
  });

  it('does not collapse the two unknowable states into an accusation', () => {
    // The whole reason this predicate is exported. A consumer branching on
    // `status !== 'met'` would light up an "unmet requirement" badge for both of
    // these, which is precisely the false accusation the constraint channel
    // exists to avoid — the app cannot tell, and must say so.
    const cannotTell: ConstraintStatus[] = ['unknown', 'noPreference'];
    for (const status of cannotTell) {
      expect(isKnockOut(check(status))).toBe(false);
      expect(check(status).status).not.toBe('met'); // the tempting wrong test
    }
  });

  it('reads only status, not the evidence fields', () => {
    // Evidence is for the renderer's sentence; it must never change the verdict.
    expect(isKnockOut({ id: 'location', status: 'unknown', posting: 'Austin, TX' })).toBe(false);
    expect(
      isKnockOut({ id: 'location', status: 'notMet', posting: undefined, candidate: undefined })
    ).toBe(true);
  });
});
