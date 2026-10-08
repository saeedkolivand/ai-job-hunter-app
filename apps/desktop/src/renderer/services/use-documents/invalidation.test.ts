import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { createMockClient, makeQueryClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useImportDocument, useRemoveDocument } from './use-documents';

describe('document mutations refresh the embedding status', () => {
  it('import and remove invalidate keys.ai.embeddingStatus', async () => {
    const queryClient = makeQueryClient();
    const spy = vi.spyOn(queryClient, 'invalidateQueries');
    const client = createMockClient({
      'documents.import': vi.fn().mockResolvedValue({}),
      'documents.remove': vi.fn().mockResolvedValue({}),
    });
    const { result } = renderHookWithClient(
      () => ({ imp: useImportDocument(), rem: useRemoveDocument() }),
      { client, queryClient }
    );
    await act(async () => {
      await result.current.imp.mutateAsync({} as never);
    });
    expect(spy).toHaveBeenCalledWith({ queryKey: keys.ai.embeddingStatus });
    spy.mockClear();
    await act(async () => {
      await result.current.rem.mutateAsync('d1');
    });
    expect(spy).toHaveBeenCalledWith({ queryKey: keys.ai.embeddingStatus });
  });
});
