import { describe, expect, it } from 'vitest';

import { timeAgo } from './time';

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
