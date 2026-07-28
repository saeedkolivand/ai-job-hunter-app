import { describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { act, waitFor } from '@testing-library/react';

import type { AiGenerationSaveRequest } from '@ajh/shared/ipc';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-ai-generations';
import { useRemoveAiGeneration, useSaveAiGeneration } from './use-ai-generations';

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

describe('useSaveAiGeneration — post-save invalidation', () => {
  const saveRequest: AiGenerationSaveRequest = {
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: '',
    jobAdLanguage: '',
    targetLanguage: '',
    mismatch: false,
    topRequirements: [],
    mode: '',
    resumeText: '',
    coverLetterText: '',
    jobAd: 'JD',
    jobUrl: 'https://acme.com/job/1',
    emailSubject: 'S',
    emailBody: 'B',
  };

  it('invalidates BOTH the generations and the applications queries', async () => {
    const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
    const client = createMockClient({ 'aiGenerations.save': save });
    const queryClient = persistentClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useSaveAiGeneration(), { client, queryClient });

    act(() => result.current.mutate(saveRequest));
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    const invalidated = invalidate.mock.calls.map(([arg]) => arg?.queryKey);
    expect(invalidated).toContainEqual(keys.aiGenerations.all);
    // The command also upserts the parent Application (ADR-0001), advancing a
    // still-`saved` row to `applied`. Without this second invalidation the
    // applications list and the detail page's status chip/timeline keep showing
    // the pre-save status until something else happens to refetch them.
    expect(invalidated).toContainEqual(keys.applications.all);
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
