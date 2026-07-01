import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as aiServices from './use-ai';
import { useAIModels } from './use-ai';

describe('use-ai services', () => {
  it('useAIModels lists models from the client', async () => {
    const client = createMockClient({
      'ai.listModels': vi.fn().mockResolvedValue(['llama3', 'mistral']),
    });
    const { result } = renderHookWithClient(() => useAIModels(), { client });
    await waitFor(() => expect(result.current.data).toEqual(['llama3', 'mistral']));
  });

  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(aiServices);
  });
});
