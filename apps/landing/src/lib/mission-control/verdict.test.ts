import { describe, expect, it } from 'vitest';

import { computeVerdict, isRedConclusion, type VerdictSignals } from './verdict';

const GREEN: VerdictSignals = {
  gatingRed: false,
  gatingKnown: true,
  criticalIssueCount: 0,
  daysSinceRelease: 2,
  openPrCount: 3,
  stalePrCount: 0,
  staleDays: 14,
};

describe('computeVerdict — ordered by severity', () => {
  it('a red build wins over everything else', () => {
    const v = computeVerdict({ ...GREEN, gatingRed: true, stalePrCount: 5, criticalIssueCount: 2 });
    expect(v.tone).toBe('bad');
    expect(v.line).toMatch(/build is red/i);
  });

  it('does NOT call the build red when no gating run is known yet', () => {
    const v = computeVerdict({ ...GREEN, gatingKnown: false, gatingRed: true });
    expect(v.line).not.toMatch(/build is red/i);
  });

  it('critical issues outrank stale PRs and stalled releases', () => {
    const v = computeVerdict({
      ...GREEN,
      criticalIssueCount: 1,
      daysSinceRelease: 40,
      stalePrCount: 9,
    });
    expect(v.tone).toBe('bad');
    expect(v.line).toMatch(/1 critical issue open/i);
  });

  it('flags a stalled release when > 21 days and nothing more urgent', () => {
    const v = computeVerdict({ ...GREEN, daysSinceRelease: 30 });
    expect(v.tone).toBe('warn');
    expect(v.line).toMatch(/stalled/i);
    expect(v.line).toMatch(/30 days/);
  });

  it('flags gathering-dust PRs (pluralization correct)', () => {
    expect(computeVerdict({ ...GREEN, stalePrCount: 1 }).line).toMatch(/1 PR is gathering dust/);
    expect(computeVerdict({ ...GREEN, stalePrCount: 3 }).line).toMatch(/3 PRs are gathering dust/);
  });

  it('is all-green when nothing is wrong', () => {
    const v = computeVerdict(GREEN);
    expect(v.tone).toBe('ok');
    expect(v.line).toMatch(/all green/i);
  });
});

describe('isRedConclusion', () => {
  it('treats failure/cancelled/timed_out as red', () => {
    expect(isRedConclusion('failure')).toBe(true);
    expect(isRedConclusion('cancelled')).toBe(true);
    expect(isRedConclusion('timed_out')).toBe(true);
  });
  it('treats success/neutral/skipped/null as not red', () => {
    expect(isRedConclusion('success')).toBe(false);
    expect(isRedConclusion('neutral')).toBe(false);
    expect(isRedConclusion('skipped')).toBe(false);
    expect(isRedConclusion(null)).toBe(false);
  });
});
