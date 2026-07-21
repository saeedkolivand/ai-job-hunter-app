// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';

import { MC_CONFIG } from './config';
import {
  authHeaders,
  fetchSnapshotStamp,
  ghGet,
  ghWrite,
  liveOrSnapshot,
  snapshotFreshnessLine,
} from './github';

const TOKEN = 'github_pat_11ABCDEFG_supersecretvalue0000';

interface FakeInit {
  ok?: boolean;
  status?: number;
  headers?: Record<string, string>;
  body?: unknown;
}
function makeRes(init: FakeInit = {}) {
  const map = new Map(Object.entries(init.headers ?? {}));
  return {
    ok: init.ok ?? true,
    status: init.status ?? 200,
    headers: { get: (key: string) => map.get(key) ?? null },
    json: () => Promise.resolve(init.body ?? {}),
  };
}
function stubFetch(res: ReturnType<typeof makeRes>) {
  const fn = vi.fn().mockResolvedValue(res);
  vi.stubGlobal('fetch', fn);
  return fn;
}
function lastUrl(fetchMock: ReturnType<typeof vi.fn>): string {
  return String(fetchMock.mock.calls.at(-1)?.[0] ?? '');
}
function lastHeaders(fetchMock: ReturnType<typeof vi.fn>): Record<string, string> {
  return (fetchMock.mock.calls.at(-1)?.[1] as { headers: Record<string, string> }).headers;
}
// Age a cached entry past the TTL so the next ghGet revalidates instead of serving it.
function expireCache(path: string): void {
  const key = MC_CONFIG.cachePrefix + path;
  const raw = localStorage.getItem(key);
  if (!raw) return;
  const entry = JSON.parse(raw) as { ts: number };
  entry.ts = Date.now() - MC_CONFIG.cacheTtlMs - 1000;
  localStorage.setItem(key, JSON.stringify(entry));
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('authHeaders', () => {
  it('carries the token only in the Authorization header when signed in', () => {
    const h = authHeaders(TOKEN);
    expect(h.Authorization).toBe(`Bearer ${TOKEN}`);
    expect(h.Accept).toBe('application/vnd.github+json');
  });

  it('sends no Authorization header when anonymous', () => {
    expect(authHeaders('').Authorization).toBeUndefined();
  });
});

describe('ghGet', () => {
  it('puts the token in the header and NEVER in the URL', async () => {
    const fetchMock = stubFetch(makeRes({ body: [{ number: 1 }] }));
    await ghGet('/pulls?state=open', TOKEN);

    const url = lastUrl(fetchMock);
    expect(url).toBe(`${MC_CONFIG.apiBase}/pulls?state=open`);
    expect(url).not.toContain(TOKEN);
    expect(lastHeaders(fetchMock).Authorization).toBe(`Bearer ${TOKEN}`);
  });

  it('omits the Authorization header entirely when anonymous', async () => {
    const fetchMock = stubFetch(makeRes({ body: {} }));
    await ghGet('/', '');
    expect(lastHeaders(fetchMock).Authorization).toBeUndefined();
    expect(lastUrl(fetchMock)).not.toContain('Bearer');
  });

  it('serves a within-TTL cache entry without any network hit', async () => {
    stubFetch(makeRes({ body: { v: 1 }, headers: { ETag: 'W/"abc"' } }));
    await ghGet<{ v: number }>('/repo', TOKEN); // primes the cache (fresh ts)

    const fetchMock = stubFetch(makeRes({ body: { v: 2 } }));
    const cachedHit = await ghGet<{ v: number }>('/repo', TOKEN);
    expect(cachedHit.v).toBe(1);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('revalidates with If-None-Match and returns the cached body on a 304 past the TTL', async () => {
    stubFetch(makeRes({ body: { v: 1 }, headers: { ETag: 'W/"abc"' } }));
    const first = await ghGet<{ v: number }>('/repo', TOKEN);
    expect(first.v).toBe(1);

    // Age the cache past the TTL so the conditional request actually fires.
    expireCache('/repo');
    const fetchMock = stubFetch(makeRes({ status: 304 }));
    const second = await ghGet<{ v: number }>('/repo', TOKEN);
    expect(second.v).toBe(1);
    expect(lastHeaders(fetchMock)['If-None-Match']).toBe('W/"abc"');
  });

  it('never leaks the token in an error message', async () => {
    stubFetch(makeRes({ ok: false, status: 500 }));
    await expect(ghGet('/pulls', TOKEN)).rejects.toThrow(/GitHub API error 500 for \/pulls/);
    await expect(ghGet('/pulls', TOKEN)).rejects.not.toThrow(new RegExp(TOKEN));
  });

  it('surfaces a rate-limit hint on a 403 with no remaining quota and no cache', async () => {
    stubFetch(makeRes({ ok: false, status: 403, headers: { 'X-RateLimit-Remaining': '0' } }));
    await expect(ghGet('/x', '')).rejects.toThrow(/rate limit/i);
  });

  it('serves stale cache on a 403 rate-limit rather than throwing', async () => {
    stubFetch(makeRes({ body: { v: 1 }, headers: { ETag: 'W/"e"' } }));
    await ghGet('/repo', TOKEN); // prime
    expireCache('/repo'); // past TTL → the next call revalidates and hits the 403

    stubFetch(makeRes({ ok: false, status: 403, headers: { 'X-RateLimit-Remaining': '0' } }));
    const out = await ghGet<{ v: number }>('/repo', TOKEN);
    expect(out.v).toBe(1);
  });
});

describe('ghWrite', () => {
  it('sends the method + body + auth header, with the token never in the URL', async () => {
    const fetchMock = stubFetch(makeRes({ ok: true, status: 200 }));
    const result = await ghWrite('/issues/5', 'PATCH', { state: 'closed' }, TOKEN);

    expect(result).toEqual({ ok: true, status: 200 });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe(`${MC_CONFIG.apiBase}/issues/5`);
    expect(url).not.toContain(TOKEN);
    expect(init.method).toBe('PATCH');
    expect(init.body).toBe(JSON.stringify({ state: 'closed' }));
    expect((init.headers as Record<string, string>).Authorization).toBe(`Bearer ${TOKEN}`);
  });
});

describe('liveOrSnapshot (the data-source seam)', () => {
  it('calls the live fetcher in live mode', async () => {
    const live = vi.fn().mockResolvedValue('LIVE');
    const out = await liveOrSnapshot({ mode: 'live' }, 'pulls', live);
    expect(out).toBe('LIVE');
    expect(live).toHaveBeenCalledOnce();
  });

  it('reads the pre-baked snapshot in snapshot mode without calling live', async () => {
    const fetchMock = stubFetch(makeRes({ ok: true, body: 'SNAP' }));
    const live = vi.fn();
    const out = await liveOrSnapshot({ mode: 'snapshot', snapshotBase: '/mc-data' }, 'pulls', live);
    expect(out).toBe('SNAP');
    expect(live).not.toHaveBeenCalled();
    expect(lastUrl(fetchMock)).toBe('/mc-data/pulls.json');
  });

  it('falls through to live when the snapshot is missing', async () => {
    stubFetch(makeRes({ ok: false, status: 404 }));
    const live = vi.fn().mockResolvedValue('LIVE');
    const out = await liveOrSnapshot({ mode: 'snapshot', snapshotBase: '/mc-data' }, 'x', live);
    expect(out).toBe('LIVE');
    expect(live).toHaveBeenCalledOnce();
  });
});

describe('fetchSnapshotStamp', () => {
  it('returns null in live mode without any fetch', async () => {
    const fetchMock = stubFetch(makeRes({ ok: true, body: { generatedAt: 'x' } }));
    expect(await fetchSnapshotStamp({ mode: 'live' })).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('reads generatedAt from the snapshot meta.json', async () => {
    const fetchMock = stubFetch(
      makeRes({ ok: true, body: { generatedAt: '2026-07-20T03:17:00Z' } })
    );
    const stamp = await fetchSnapshotStamp({ mode: 'snapshot', snapshotBase: '/metrics' });
    expect(stamp).toBe('2026-07-20T03:17:00Z');
    expect(lastUrl(fetchMock)).toBe('/metrics/meta.json');
  });

  it('returns null when the snapshot meta is absent (404)', async () => {
    stubFetch(makeRes({ ok: false, status: 404 }));
    expect(await fetchSnapshotStamp({ mode: 'snapshot', snapshotBase: '/metrics' })).toBeNull();
  });

  it('returns null when meta.json lacks a generatedAt', async () => {
    stubFetch(makeRes({ ok: true, body: {} }));
    expect(await fetchSnapshotStamp({ mode: 'snapshot', snapshotBase: '/metrics' })).toBeNull();
  });
});

describe('snapshotFreshnessLine', () => {
  const NOW = Date.parse('2026-07-20T12:00:00Z');

  it('formats a recent snapshot as a muted relative-time line', () => {
    const threeHoursAgo = new Date(NOW - 3 * 60 * 60 * 1000).toISOString();
    expect(snapshotFreshnessLine(threeHoursAgo, NOW)).toBe(
      'snapshot from 3 hours ago · data refreshes nightly'
    );
  });

  it('formats a multi-day-old snapshot in days', () => {
    const twoDaysAgo = new Date(NOW - 2 * 24 * 60 * 60 * 1000).toISOString();
    expect(snapshotFreshnessLine(twoDaysAgo, NOW)).toBe(
      'snapshot from 2 days ago · data refreshes nightly'
    );
  });

  it('returns null for an unparseable timestamp', () => {
    expect(snapshotFreshnessLine('not-a-date', NOW)).toBeNull();
  });
});
