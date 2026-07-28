import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { isStale, nextActionLabel, staleDays } from './stale';

/** 2026-07-28 14:30 local — mid-afternoon, so "today" has clearly started. */
const NOW = new Date(2026, 6, 28, 14, 30, 0).getTime();
const startOfToday = new Date(2026, 6, 28, 0, 0, 0).getTime();
const DAY = 24 * 60 * 60 * 1000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});
afterEach(() => vi.useRealTimers());

describe('nextActionLabel', () => {
  it('is "none" without a reminder', () => {
    expect(nextActionLabel(undefined)).toBe('none');
    expect(nextActionLabel(0)).toBe('none');
  });

  // The headline regression: `<input type="date">` stores LOCAL MIDNIGHT, so a
  // reminder set for today is already "in the past" by 00:01 — it was announced
  // as overdue on the very day it was due, in the badge and the strip counter.
  it('is "upcoming" for a reminder due TODAY, at any time of day', () => {
    expect(nextActionLabel(startOfToday)).toBe('upcoming');
    // Even at 23:59 the day has not passed.
    vi.setSystemTime(new Date(2026, 6, 28, 23, 59, 59).getTime());
    expect(nextActionLabel(startOfToday)).toBe('upcoming');
  });

  it('is "overdue" only once the due day has fully passed', () => {
    expect(nextActionLabel(startOfToday - 1)).toBe('overdue');
    expect(nextActionLabel(startOfToday - DAY)).toBe('overdue');
  });

  it('is "upcoming" for a future reminder', () => {
    expect(nextActionLabel(startOfToday + DAY)).toBe('upcoming');
    expect(nextActionLabel(NOW + 60_000)).toBe('upcoming');
  });

  it('flips to overdue exactly at the next local midnight', () => {
    expect(nextActionLabel(startOfToday)).toBe('upcoming');
    vi.setSystemTime(new Date(2026, 6, 29, 0, 0, 0).getTime());
    expect(nextActionLabel(startOfToday)).toBe('overdue');
  });
});

describe('isStale', () => {
  it('is true at or past the threshold and false before it', () => {
    expect(isStale(NOW - 30 * DAY)).toBe(true);
    expect(isStale(NOW - 31 * DAY)).toBe(true);
    expect(isStale(NOW - 29 * DAY)).toBe(false);
    expect(isStale(NOW)).toBe(false);
  });

  it('honours an explicit threshold', () => {
    expect(isStale(NOW - 2 * DAY, DAY)).toBe(true);
    expect(isStale(NOW - 2 * DAY, 7 * DAY)).toBe(false);
  });
});

describe('staleDays', () => {
  it('floors the elapsed whole days', () => {
    expect(staleDays(NOW)).toBe(0);
    expect(staleDays(NOW - DAY - 1)).toBe(1);
    expect(staleDays(NOW - 14 * DAY)).toBe(14);
  });
});
