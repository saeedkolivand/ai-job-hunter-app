import { afterEach, describe, expect, it, vi } from 'vitest';

import { getTimeGreeting } from './greeting';

describe('getTimeGreeting', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it.each([
    [6, 'Good morning'],
    [11, 'Good morning'],
    [12, 'Good afternoon'],
    [17, 'Good afternoon'],
    [18, 'Good evening'],
    [23, 'Good evening'],
  ])('returns the right greeting at hour %i', (hour, expected) => {
    vi.useFakeTimers();
    const d = new Date();
    d.setHours(hour, 0, 0, 0);
    vi.setSystemTime(d);
    expect(getTimeGreeting()).toBe(expected);
  });
});
