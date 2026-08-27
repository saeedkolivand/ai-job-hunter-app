import { describe, expect, it } from 'vitest';

import { formatSalaryRange } from './format-salary';

// A fixed locale keeps the group separators deterministic across CI hosts.
const EN = 'en-US';

describe('formatSalaryRange', () => {
  it('returns null when neither bound is present', () => {
    expect(formatSalaryRange(undefined, undefined, 'EUR', EN)).toBeNull();
    expect(formatSalaryRange(undefined, undefined, undefined, EN)).toBeNull();
  });

  it('formats both bounds with the currency, joined by an en dash', () => {
    expect(formatSalaryRange(60000, 80000, 'EUR', EN)).toBe('€60,000–€80,000');
  });

  it('collapses an equal min/max to one figure', () => {
    expect(formatSalaryRange(70000, 70000, 'USD', EN)).toBe('$70,000');
  });

  it('renders a single bound when only one is known', () => {
    expect(formatSalaryRange(90000, undefined, 'USD', EN)).toBe('$90,000');
    expect(formatSalaryRange(undefined, 120000, 'USD', EN)).toBe('$120,000');
  });

  it('drops the fractional part (salary chips are whole numbers)', () => {
    expect(formatSalaryRange(59999.6, undefined, 'USD', EN)).toBe('$60,000');
  });

  it('falls back to a plain number when the currency code is junk (never blanks the row)', () => {
    expect(formatSalaryRange(60000, 80000, 'not-a-currency', EN)).toBe('60,000–80,000');
    expect(formatSalaryRange(60000, undefined, '', EN)).toBe('60,000');
    expect(formatSalaryRange(60000, undefined, '   ', EN)).toBe('60,000');
  });

  it('formats without a currency symbol when no code is supplied', () => {
    expect(formatSalaryRange(60000, 80000, undefined, EN)).toBe('60,000–80,000');
  });

  it('handles zero as a real bound, not a missing one', () => {
    expect(formatSalaryRange(0, 40000, 'USD', EN)).toBe('$0–$40,000');
  });
});
