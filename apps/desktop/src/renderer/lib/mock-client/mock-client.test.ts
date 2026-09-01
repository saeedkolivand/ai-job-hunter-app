import { describe, expect, it } from 'vitest';

import type { ScrapeProgressEvent } from '@ajh/shared';

import { createMockClient, emitScrapeProgress } from './mock-client';

describe('createMockClient', () => {
  it('returns a fully-populated client with every namespace', () => {
    const client = createMockClient();
    expect(Object.keys(client).sort()).toContain('system');
    expect(client.ai).toBeDefined();
    expect(client.documents).toBeDefined();
    expect(client.autopilot).toBeDefined();
  });

  it('provides sensible default resolved values', async () => {
    const client = createMockClient();
    await expect(client.system.getProtocolVersion()).resolves.toBe('1.1.0');
    await expect(client.system.getLocale()).resolves.toBe('en');
    await expect(client.jobs.list()).resolves.toEqual([]);
    await expect(client.documents.list()).resolves.toEqual([]);
    await expect(client.credentials.available()).resolves.toBe(false);
    await expect(client.linkedin.getStatus()).resolves.toEqual({ connected: false });
  });

  // Regression: `hybridSearch` used to be a bare `noop` (resolves `undefined`)
  // while `usePostingsSearch`'s success handler reads `data.outcome`/
  // `data.hits` unconditionally — every mock-backed search threw.
  it('resolves a valid HybridSearchResult shape from hybridSearch, not undefined', async () => {
    const client = createMockClient();
    const result = await client.scrape.hybridSearch({
      queryId: 'search-test',
      query: 'engineer',
    });
    expect(result.outcome).toBe('ok');
    expect(result.hits).toEqual([]);
    expect(result.arms).toEqual({ lexical: 'skipped', dense: 'skipped', rerank: 'skipped' });
    expect(result.corpusSize).toBe(0);
  });

  it('returns sync unsubscribe handles for event subscriptions', () => {
    const client = createMockClient();
    expect(typeof client.ai.onStream(() => {})).toBe('function');
    expect(typeof client.jobs.onEvent(() => {})).toBe('function');
    expect(typeof client.updater.onStatus(() => {})).toBe('function');
  });

  it('fans scrape progress out to registered handlers and stops after unsubscribe', () => {
    const client = createMockClient();
    const seen: ScrapeProgressEvent[] = [];
    const off = client.scrape.onProgress((e) => seen.push(e));

    emitScrapeProgress(client, { jobId: 'job-1', progress: 0.5 });
    expect(seen).toEqual([{ jobId: 'job-1', progress: 0.5 }]);

    off();
    emitScrapeProgress(client, { jobId: 'job-1', progress: 1 });
    expect(seen).toHaveLength(1);
  });

  it('shallow-merges namespace overrides', async () => {
    const client = createMockClient({
      system: { getVersion: async () => '9.9.9' },
    });
    await expect(client.system.getVersion()).resolves.toBe('9.9.9');
    // Non-overridden methods on the same namespace remain intact.
    await expect(client.system.getProtocolVersion()).resolves.toBe('1.1.0');
  });
});
