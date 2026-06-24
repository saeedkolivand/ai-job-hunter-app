import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-match';
import { useInvalidateMatchBatch } from './use-match';

describe('use-match services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });

  it('useInvalidateMatchBatch calls invalidateQueries with the match-batch key', async () => {
    const { queryClient, result } = renderHookWithClient(() => useInvalidateMatchBatch());
    const spy = vi.spyOn(queryClient, 'invalidateQueries').mockResolvedValue();

    await act(async () => {
      await result.current();
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ['match-batch'] });
  });
});
