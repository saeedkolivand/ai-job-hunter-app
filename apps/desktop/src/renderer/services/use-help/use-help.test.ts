import { describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { act } from '@testing-library/react';

import { keys } from '@/services/query-client';
import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-help';
import { useFetchHelpDataSources } from './use-help';

describe('use-help services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

const STATUS = { documents: { total: 3, indexedInActiveSpace: 3, stale: 0 } };
const INTERACTIONS = [{ interactionType: 'viewed' }];
const APPLICATIONS = [{ id: 'a1', title: 'Senior Engineer' }];
const AUTOPILOTS = [{ id: 'ap1' }];

describe('useFetchHelpDataSources', () => {
  it('caches its four reads under the SHARED query keys, never private ones', async () => {
    const client = createMockClient({
      'ai.embeddingStatus': vi.fn().mockResolvedValue(STATUS),
      'scrape.listInteractions': vi.fn().mockResolvedValue(INTERACTIONS),
      'applications.list': vi.fn().mockResolvedValue(APPLICATIONS),
      'autopilot.list': vi.fn().mockResolvedValue(AUTOPILOTS),
    });
    // Not `makeQueryClient()`: its `gcTime: 0` collects an unobserved
    // `fetchQuery` result the moment it lands, which is the one thing this
    // test needs to still be there.
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { result } = renderHookWithClient(() => useFetchHelpDataSources(), {
      client,
      queryClient,
    });

    await act(async () => {
      await result.current();
    });

    // The whole reason this is `fetchQuery` on the app's own keys rather than
    // four private reads: the help chat is one more READER of these queries, so
    // what it fetches is warm for `useApplications` / `useEmbeddingStatus` /
    // `useInteractions` / `useAutopilots` in turn. A key of its own would leave
    // every one of these `undefined` and quietly double the IPC traffic.
    expect(queryClient.getQueryData(keys.applications.all)).toEqual(APPLICATIONS);
    expect(queryClient.getQueryData(keys.ai.embeddingStatus)).toEqual(STATUS);
    expect(queryClient.getQueryData(keys.autopilot.all)).toEqual(AUTOPILOTS);
    // The optional segment is part of the key the typed hook uses — a bare
    // `['postings','interactions']` would be a different cache entry.
    expect(queryClient.getQueryData(keys.postings.interactions(undefined))).toEqual(INTERACTIONS);
  });
});
