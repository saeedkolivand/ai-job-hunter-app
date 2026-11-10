import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import type { SyncStatus } from '@ajh/shared';

import { createMockClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useSyncDataRefresh } from './use-sync-data-refresh';

describe('app-global received-data refresh', () => {
  it('refreshes a cached contact profile without navigating or restarting', async () => {
    const client = createMockClient({
      'sync.status': vi.fn().mockResolvedValue({ enabled: false, insecure: false }),
    });
    const { queryClient } = renderHookWithClient(() => useSyncDataRefresh(), { client });
    await waitFor(() => expect(queryClient.getQueryData(keys.sync.status)).toBeDefined());
    queryClient.setQueryDefaults(keys.contactProfile.all, { gcTime: Infinity });
    queryClient.setQueryDefaults(keys.credentials.all, { gcTime: Infinity });
    queryClient.setQueryDefaults(keys.ai.models, { gcTime: Infinity });
    queryClient.setQueryData(keys.ai.models, ['local-model']);
    queryClient.setQueryData(keys.contactProfile.all, { fullName: 'Old name' });
    queryClient.setQueryData(keys.credentials.all, ['device-only']);
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    const received: SyncStatus = {
      enabled: true,
      insecure: false,
      lastSyncAt: 1234,
      lastReport: { pulled: 1, applied: 1, pushed: 0, held: 0 },
    };
    await act(() => queryClient.setQueryData(keys.sync.status, received));
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({ queryKey: keys.contactProfile.all })
    );
    expect(queryClient.getQueryState(keys.contactProfile.all)?.isInvalidated).toBe(true);
    for (const queryKey of [
      keys.applications.all,
      keys.documents.all,
      keys.aiGenerations.all,
      keys.contactProfile.all,
      keys.jobPreferences.all,
      keys.ai.activeConfig,
      keys.ai.stageOverrides,
      keys.ai.spend,
      keys.autopilot.all,
      keys.postings.all,
      keys.referrals.all,
      keys.discovery.all,
      keys.pipeline.all,
    ])
      expect(invalidate).toHaveBeenCalledWith({ queryKey });
    expect(queryClient.getQueryState(keys.ai.models)?.isInvalidated).toBe(false);
    expect(queryClient.getQueryState(keys.credentials.all)?.isInvalidated).toBe(false);
    expect(queryClient.getQueryState(keys.sync.status)?.isInvalidated).toBe(false);
    const calls = invalidate.mock.calls.length;
    await act(() => queryClient.setQueryData(keys.sync.status, { ...received }));
    expect(invalidate).toHaveBeenCalledTimes(calls);
    await act(() => queryClient.setQueryData(keys.sync.status, { ...received, lastSyncAt: 1235 }));
    await waitFor(() => expect(invalidate.mock.calls.length).toBeGreaterThan(calls));
  });

  it.each<[string, Partial<SyncStatus>]>([
    [
      'disabled, nothing applied',
      {
        enabled: false,
        lastSyncAt: 1234,
        lastReport: { pulled: 2, applied: 0, pushed: 0, held: 2 },
      },
    ],
    [
      'enabled, nothing applied',
      {
        enabled: true,
        lastSyncAt: 1234,
        lastReport: { pulled: 2, applied: 0, pushed: 0, held: 2 },
      },
    ],
    [
      'applied without a sync time',
      { enabled: true, lastReport: { pulled: 1, applied: 1, pushed: 0, held: 0 } },
    ],
    ['no last report', { enabled: true, lastSyncAt: 1234 }],
  ])('does not refresh stores (%s)', async (_, status) => {
    const client = createMockClient({
      'sync.status': vi.fn().mockResolvedValue({ insecure: false, ...status }),
    });
    const { queryClient } = renderHookWithClient(() => useSyncDataRefresh(), { client });
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    await waitFor(() => expect(queryClient.getQueryData(keys.sync.status)).toBeDefined());
    expect(invalidate).not.toHaveBeenCalled();
  });
});
