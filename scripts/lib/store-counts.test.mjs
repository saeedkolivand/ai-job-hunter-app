import { describe, expect, it } from 'vitest';

import {
  parseChromeUsers,
  parseFirefoxUsers,
  snapInstalledBase,
  sumAcquisitions,
  totalInstalls,
} from './store-counts.mjs';

describe('parseChromeUsers', () => {
  it('parses the plain form', () => {
    expect(parseChromeUsers('<span>21 users</span>')).toBe(21);
  });

  it('parses a comma-grouped count', () => {
    expect(parseChromeUsers('<span>1,234 users</span>')).toBe(1234);
  });

  it('applies the K multiplier', () => {
    expect(parseChromeUsers('10K+ users')).toBe(10_000);
  });

  it('applies the M multiplier', () => {
    expect(parseChromeUsers('2.5M users')).toBe(2_500_000);
  });

  it('finds the figure amid surrounding HTML noise', () => {
    const html = '<div class="x"><b>21 users</b></div><script>ga("send")</script>';
    expect(parseChromeUsers(html)).toBe(21);
  });

  it('returns null when there is no match', () => {
    expect(parseChromeUsers('<div>Overview</div>')).toBeNull();
    expect(parseChromeUsers('')).toBeNull();
    expect(parseChromeUsers(undefined)).toBeNull();
  });
});

describe('parseFirefoxUsers', () => {
  it('returns the integer count', () => {
    expect(parseFirefoxUsers({ average_daily_users: 1 })).toBe(1);
  });

  it('returns null when the field is missing', () => {
    expect(parseFirefoxUsers({})).toBeNull();
    expect(parseFirefoxUsers(null)).toBeNull();
  });

  it('returns null for negative or NaN values', () => {
    expect(parseFirefoxUsers({ average_daily_users: -1 })).toBeNull();
    expect(parseFirefoxUsers({ average_daily_users: Number.NaN })).toBeNull();
  });
});

describe('snapInstalledBase', () => {
  const metric = {
    buckets: ['2026-09-08', '2026-09-09', '2026-09-10'],
    series: [
      { name: 'stable', values: [1, 2, 3] },
      { name: 'edge', values: [null, 1, 4] },
    ],
  };

  it('unwraps a metrics-wrapped payload', () => {
    expect(snapInstalledBase({ metrics: [metric] })).toBe(7); // 3 + 4
  });

  it('accepts a bare metric object', () => {
    expect(snapInstalledBase(metric)).toBe(7);
  });

  it('sums only the LAST bucket, not every bucket', () => {
    const singleSeries = { buckets: ['a', 'b'], series: [{ name: 's', values: [100, 5] }] };
    expect(snapInstalledBase(singleSeries)).toBe(5);
  });

  it('counts a null value as 0 rather than skipping the series', () => {
    const withNull = {
      buckets: ['a'],
      series: [
        { name: 's', values: [null] },
        { name: 't', values: [3] },
      ],
    };
    expect(snapInstalledBase(withNull)).toBe(3);
  });

  it('returns null for an unrecognized shape', () => {
    expect(snapInstalledBase({})).toBeNull();
    expect(snapInstalledBase({ metrics: [] })).toBeNull();
    expect(snapInstalledBase({ series: [], buckets: [] })).toBeNull();
    expect(snapInstalledBase(null)).toBeNull();
  });
});

describe('sumAcquisitions', () => {
  it('sums acquisitionQuantity across two pages', () => {
    const pages = [
      { Value: [{ acquisitionQuantity: 2 }, { acquisitionQuantity: 3 }] },
      { Value: [{ acquisitionQuantity: 5 }] },
    ];
    expect(sumAcquisitions(pages)).toBe(10);
  });

  it('ignores rows missing the field', () => {
    const pages = [{ Value: [{ acquisitionQuantity: 4 }, { date: '2026-09-09' }] }];
    expect(sumAcquisitions(pages)).toBe(4);
  });

  it('returns null when no page has a Value array', () => {
    expect(sumAcquisitions([{}])).toBeNull();
    expect(sumAcquisitions([])).toBeNull();
    expect(sumAcquisitions(undefined)).toBeNull();
  });
});

describe('totalInstalls', () => {
  it('ignores nulls and sums the rest', () => {
    expect(totalInstalls({ github: 10, msStore: null, snap: 5, chrome: 2, firefox: null })).toBe(
      17
    );
  });

  it('returns 0 when every part is null', () => {
    expect(
      totalInstalls({ github: null, msStore: null, snap: null, chrome: null, firefox: null })
    ).toBe(0);
  });
});
