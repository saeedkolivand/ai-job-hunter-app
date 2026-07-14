import { describe, expect, it } from 'vitest';

import { parseCalendarOrIsoDate, timeAgo } from './time';

describe('timeAgo', () => {
  const NOW = new Date('2024-06-01T12:00:00.000Z').getTime();

  it('returns empty string for an unparseable input', () => {
    expect(timeAgo('not-a-date', NOW)).toBe('');
  });

  it('formats seconds ago', () => {
    const result = timeAgo(NOW - 30_000, NOW, 'en');
    expect(result).toBeTruthy();
    expect(typeof result).toBe('string');
  });

  it('formats minutes ago', () => {
    const result = timeAgo(NOW - 5 * 60_000, NOW, 'en');
    expect(result).toBeTruthy();
  });

  it('formats hours ago', () => {
    const result = timeAgo(NOW - 3 * 60 * 60_000, NOW, 'en');
    expect(result).toBeTruthy();
  });

  it('formats days ago', () => {
    const result = timeAgo(NOW - 3 * 24 * 60 * 60_000, NOW, 'en');
    expect(result).toBeTruthy();
  });

  it('accepts a Date object', () => {
    const result = timeAgo(new Date(NOW - 60_000), NOW, 'en');
    expect(result).toBeTruthy();
  });

  it('accepts an ISO date string', () => {
    const result = timeAgo(new Date(NOW - 60_000).toISOString(), NOW, 'en');
    expect(result).toBeTruthy();
  });

  it('uses the locale parameter for formatting', () => {
    const en = timeAgo(NOW - 5 * 60_000, NOW, 'en');
    expect(typeof en).toBe('string');
    expect(en.length).toBeGreaterThan(0);
  });

  it('falls back gracefully when no locale is provided', () => {
    const result = timeAgo(NOW - 60_000, NOW);
    expect(typeof result).toBe('string');
  });
});

describe('parseCalendarOrIsoDate', () => {
  it('reads a date-only string as the local calendar date, not UTC midnight', () => {
    // `new Date('2026-01-01')` parses as UTC midnight, which is 2025-12-31 in
    // any timezone west of UTC — the bug this guards against.
    const date = parseCalendarOrIsoDate('2026-01-01');
    expect(date.getFullYear()).toBe(2026);
    expect(date.getMonth()).toBe(0);
    expect(date.getDate()).toBe(1);
  });

  it('passes a full ISO timestamp through unchanged', () => {
    const iso = '2026-01-01T10:30:00.000Z';
    expect(parseCalendarOrIsoDate(iso).toISOString()).toBe(new Date(iso).toISOString());
  });
});
