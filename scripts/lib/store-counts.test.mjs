import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  collectStoreCounts,
  fetchChromeUsers,
  fetchFirefoxUsers,
  fetchMsStoreAcquisitions,
  fetchSnapInstalledBase,
  parseChromeUsers,
  parseFirefoxUsers,
  snapInstalledBase,
  sumAcquisitions,
  totalInstalls,
} from './store-counts.mjs';

afterEach(() => {
  vi.unstubAllGlobals();
});

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

  it('returns null above the sanity ceiling', () => {
    expect(parseChromeUsers('10M users')).toBeNull();
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

  it('walks back one bucket when the latest is entirely null', () => {
    const m = {
      buckets: ['a', 'b'],
      series: [
        { name: 's', values: [5, null] },
        { name: 't', values: [2, null] },
      ],
    };
    expect(snapInstalledBase(m)).toBe(7);
  });

  it('returns null when every bucket is null', () => {
    const m = { buckets: ['a', 'b'], series: [{ name: 's', values: [null, null] }] };
    expect(snapInstalledBase(m)).toBeNull();
  });

  it("returns null when status is a string other than 'OK'", () => {
    const m = { status: 'error', buckets: ['a'], series: [{ name: 's', values: [3] }] };
    expect(snapInstalledBase(m)).toBeNull();
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

describe('fetchMsStoreAcquisitions', () => {
  it('returns null and never calls fetch when secrets are missing', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);

    expect(await fetchMsStoreAcquisitions({})).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('resolves a relative @nextLink to an absolute manage.devcenter.microsoft.com URL and sums both pages', async () => {
    const urls = [];
    let n = 0;
    const fetchMock = vi.fn(async (url) => {
      urls.push(String(url));
      n += 1;
      if (n === 1) return { ok: true, json: async () => ({ access_token: 'tok' }) };
      if (n === 2) {
        return {
          ok: true,
          json: async () => ({
            Value: [{ acquisitionQuantity: 3 }],
            '@nextLink': 'appacquisitions?applicationId=x&skip=10000',
          }),
        };
      }
      return { ok: true, json: async () => ({ Value: [{ acquisitionQuantity: 4 }] }) };
    });
    vi.stubGlobal('fetch', fetchMock);

    const env = { MSSTORE_TENANT_ID: 't', MSSTORE_CLIENT_ID: 'c', MSSTORE_CLIENT_SECRET: 's' };
    expect(await fetchMsStoreAcquisitions(env)).toBe(7);
    expect(urls[2]).toMatch(/^https:\/\/manage\.devcenter\.microsoft\.com\//);
  });

  it('returns null when a page mid-pagination responds non-2xx', async () => {
    let n = 0;
    const fetchMock = vi.fn(async () => {
      n += 1;
      if (n === 1) return { ok: true, json: async () => ({ access_token: 'tok' }) };
      if (n === 2) {
        return {
          ok: true,
          json: async () => ({ Value: [{ acquisitionQuantity: 1 }], '@nextLink': 'more' }),
        };
      }
      return { ok: false, json: async () => ({}) };
    });
    vi.stubGlobal('fetch', fetchMock);

    const env = { MSSTORE_TENANT_ID: 't', MSSTORE_CLIENT_ID: 'c', MSSTORE_CLIENT_SECRET: 's' };
    expect(await fetchMsStoreAcquisitions(env)).toBeNull();
  });

  it('returns null and never fetches a foreign-origin absolute @nextLink', async () => {
    let n = 0;
    const hosts = [];
    const fetchMock = vi.fn(async (url) => {
      hosts.push(new URL(String(url)).host);
      n += 1;
      if (n === 1) return { ok: true, json: async () => ({ access_token: 'tok' }) };
      return {
        ok: true,
        json: async () => ({
          Value: [{ acquisitionQuantity: 1 }],
          '@nextLink': 'https://evil.example.com/steal',
        }),
      };
    });
    vi.stubGlobal('fetch', fetchMock);

    const env = { MSSTORE_TENANT_ID: 't', MSSTORE_CLIENT_ID: 'c', MSSTORE_CLIENT_SECRET: 's' };
    expect(await fetchMsStoreAcquisitions(env)).toBeNull();
    expect(hosts).not.toContain('evil.example.com');
  });

  it('returns null when more pages exist than the page cap', async () => {
    let n = 0;
    const fetchMock = vi.fn(async () => {
      n += 1;
      if (n === 1) return { ok: true, json: async () => ({ access_token: 'tok' }) };
      return {
        ok: true,
        json: async () => ({
          Value: [{ acquisitionQuantity: 1 }],
          '@nextLink': `more?page=${n}`,
        }),
      };
    });
    vi.stubGlobal('fetch', fetchMock);

    const env = { MSSTORE_TENANT_ID: 't', MSSTORE_CLIENT_ID: 'c', MSSTORE_CLIENT_SECRET: 's' };
    expect(await fetchMsStoreAcquisitions(env)).toBeNull();
  });
});

describe('fetchFirefoxUsers', () => {
  it('returns null on a non-2xx response', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: false, json: async () => ({}) }))
    );
    expect(await fetchFirefoxUsers('ai-job-hunter')).toBeNull();
  });
});

describe('fetchChromeUsers', () => {
  it('returns null when fetch throws', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('offline');
      })
    );
    expect(await fetchChromeUsers('id')).toBeNull();
  });
});

describe('fetchSnapInstalledBase', () => {
  it('returns null when SNAPCRAFT_STORE_CREDENTIALS is unset', async () => {
    expect(await fetchSnapInstalledBase('ai-job-hunter', {})).toBeNull();
  });
});

describe('collectStoreCounts', () => {
  it('resolves (never rejects) with nulls for the HTTP stores when fetch throws', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('offline');
      })
    );

    const result = await collectStoreCounts({});
    expect(result).toEqual({ msStore: null, snap: null, chrome: null, firefox: null });
  });
});
