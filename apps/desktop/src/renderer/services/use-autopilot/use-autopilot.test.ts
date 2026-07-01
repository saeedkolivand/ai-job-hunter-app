import { describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-autopilot';
import { useRemoveAutopilot } from './use-autopilot';

// gcTime: Infinity so cache seeded without an active observer is not collected.
const persistentClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: Infinity },
      mutations: { retry: false },
    },
  });

describe('use-autopilot services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useRemoveAutopilot — optimistic delete', () => {
  it('removes the card before the backend resolves, then rolls back on error', async () => {
    let reject!: (e: unknown) => void;
    const remove = vi.fn(() => new Promise((_res, rej) => (reject = rej)));
    const list = vi.fn().mockResolvedValue([{ _id: 'a' }, { _id: 'b' }]);
    const client = createMockClient({ 'autopilot.remove': remove, 'autopilot.list': list });
    const queryClient = persistentClient();
    queryClient.setQueryData(keys.autopilot.all, [{ _id: 'a' }, { _id: 'b' }]);

    const { result } = renderHookWithClient(() => useRemoveAutopilot(), { client, queryClient });

    act(() => result.current.mutate('a'));

    await waitFor(() =>
      expect(queryClient.getQueryData(keys.autopilot.all)).toEqual([{ _id: 'b' }])
    );

    act(() => reject(new Error('boom')));
    await waitFor(() =>
      expect(queryClient.getQueryData(keys.autopilot.all)).toEqual([{ _id: 'a' }, { _id: 'b' }])
    );
  });
});
