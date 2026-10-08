import { afterEach, describe, expect, it, vi } from 'vitest';

import type { TFunction } from '@ajh/translations';

import { getTimeGreeting } from './greeting';

describe('getTimeGreeting', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it.each([
    [6, 'nav.greeting.morning'],
    [11, 'nav.greeting.morning'],
    [12, 'nav.greeting.afternoon'],
    [17, 'nav.greeting.afternoon'],
    [18, 'nav.greeting.evening'],
    [23, 'nav.greeting.evening'],
  ])('returns the right greeting at hour %i', (hour, expected) => {
    vi.useFakeTimers();
    const d = new Date();
    d.setHours(hour, 0, 0, 0);
    vi.setSystemTime(d);
    expect(getTimeGreeting(((k: string) => k) as TFunction)).toBe(expected);
  });
});
