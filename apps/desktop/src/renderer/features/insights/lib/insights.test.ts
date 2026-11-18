import { describe, expect, it } from 'vitest';

import type { InsightsOutcomeGroup } from '@ajh/shared';

import { groupsOf, HEADLINE_MIN_DECIDED, headlineOf } from './insights';

const g = (
  dimension: InsightsOutcomeGroup['dimension'],
  key: string,
  positiveReply: number,
  noReply = 0
): InsightsOutcomeGroup => ({
  dimension,
  key,
  counts: { positiveReply, rejected: 0, noReply, pending: 0 },
  rates: null,
});

describe('headlineOf', () => {
  it('sums only the month dimension so each application counts once', () => {
    const h = headlineOf([
      g('board', 'a', 100),
      g('month', '2026-09', 2, 1),
      g('month', '2026-10', 1, 3),
    ]);
    expect(h.counts.positiveReply).toBe(3);
    expect(h.decided).toBe(7);
    expect(h.rate).toBeCloseTo(3 / 7);
  });

  it('withholds the rate below the minimum decided sample', () => {
    const h = headlineOf([g('month', '2026-10', 1, HEADLINE_MIN_DECIDED - 2)]);
    expect(h.rate).toBeNull();
  });
});

describe('groupsOf', () => {
  it('keeps months chronological and orders the rest by size, biggest first', () => {
    const all = [
      g('month', '2026-10', 1),
      g('month', '2026-09', 1),
      g('board', 'small', 1),
      g('board', 'big', 5),
    ];
    expect(groupsOf(all, 'month').map((x) => x.key)).toEqual(['2026-09', '2026-10']);
    expect(groupsOf(all, 'board').map((x) => x.key)).toEqual(['big', 'small']);
  });

  it('sorts the unknown group last whatever its size', () => {
    const all = [
      g('template', 'unknown', 50),
      g('template', 'classic', 2),
      g('template', 'modern', 1),
    ];
    expect(groupsOf(all, 'template').map((x) => x.key)).toEqual(['classic', 'modern', 'unknown']);
  });
});
