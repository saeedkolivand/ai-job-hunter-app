import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useFormatRelativeTime } from './use-format-relative-time';

/** Echoes the i18n key plus its interpolation values, so a test can assert both. */
const t = ((key: string, vars?: Record<string, unknown>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key) as never;

const NOW = 1_750_000_000_000;
const DAY = 24 * 60 * 60 * 1000;

describe('useFormatRelativeTime', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('keeps the tiers contiguous across the week to month handover', () => {
    // Days 28-29 used to fall through `weeks < 4` into the month tier, where
    // `months = floor(days / 30)` is still 0 — rendering "0mo ago".
    const format = useFormatRelativeTime(t);
    const at = (days: number) => format(NOW - days * DAY);

    expect(at(27)).toBe('jobs.timeWeeksAgo:{"w":3}');
    expect(at(28)).toBe('jobs.timeWeeksAgo:{"w":4}');
    expect(at(29)).toBe('jobs.timeWeeksAgo:{"w":4}');
    expect(at(30)).toBe('jobs.timeMonthsAgo:{"m":1}');
    expect(at(75)).toBe('jobs.timeMonthsAgo:{"m":2}');
  });

  it('never interpolates a zero count in any tier', () => {
    const format = useFormatRelativeTime(t);

    for (let days = 1; days <= 400; days += 1) {
      expect(format(NOW - days * DAY)).not.toMatch(/:\{"[a-z]":0\}$/);
    }
  });

  it('formats the sub-week tiers unchanged', () => {
    const format = useFormatRelativeTime(t);

    expect(format(NOW)).toBe('jobs.timeJustNow');
    expect(format(NOW - 5 * 60_000)).toBe('jobs.timeMinutesAgo:{"m":5}');
    expect(format(NOW - 3 * 60 * 60_000)).toBe('jobs.timeHoursAgo:{"h":3}');
    expect(format(NOW - 6 * DAY)).toBe('jobs.timeDaysAgo:{"d":6}');
    expect(format(undefined)).toBe('');
  });

  it('uses the resume namespace when asked', () => {
    const format = useFormatRelativeTime(t, 'resumes.relativeTime');

    expect(format(NOW - 29 * DAY)).toBe('resumes.relativeTime.weeksAgo:{"w":4}');
  });
});
