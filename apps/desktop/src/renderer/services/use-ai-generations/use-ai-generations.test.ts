import { describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-ai-generations';
import { useRemoveAiGeneration } from './use-ai-generations';

// gcTime: Infinity so cache seeded without an active observer is not collected.
const persistentClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: Infinity },
      mutations: { retry: false },
    },
  });

describe('use-ai-generations services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useRemoveAiGeneration — optimistic delete', () => {
  it('removes the item before the backend resolves, then rolls back on error', async () => {
    let reject!: (e: unknown) => void;
    const remove = vi.fn(() => new Promise((_res, rej) => (reject = rej)));
    const list = vi.fn().mockResolvedValue([{ id: 'a' }, { id: 'b' }]);
    const client = createMockClient({
      'aiGenerations.remove': remove,
      'aiGenerations.list': list,
    });
    const queryClient = persistentClient();
    queryClient.setQueryData(keys.aiGenerations.all, [{ id: 'a' }, { id: 'b' }]);

    const { result } = renderHookWithClient(() => useRemoveAiGeneration(), { client, queryClient });

    act(() => result.current.mutate('a'));

    // Optimistic: 'a' is gone immediately, before remove() ever resolves.
    await waitFor(() =>
      expect(queryClient.getQueryData(keys.aiGenerations.all)).toEqual([{ id: 'b' }])
    );

    // Backend fails → the snapshot is restored.
    act(() => reject(new Error('boom')));
    await waitFor(() =>
      expect(queryClient.getQueryData(keys.aiGenerations.all)).toEqual([{ id: 'a' }, { id: 'b' }])
    );
  });
});
