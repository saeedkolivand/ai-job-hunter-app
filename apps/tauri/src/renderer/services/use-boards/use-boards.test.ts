import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

import { createMockClient, renderHookWithClient } from '@/test-support';

import * as mod from './use-boards';
import { useBoardsCatalog } from './use-boards';

// ── smoke test ────────────────────────────────────────────────────────────────

describe('use-boards services', () => {
  it('renders every exported hook without crashing', async () => {
    const { exerciseServiceHooks } = await import('@/test-support');
    await exerciseServiceHooks(mod);
  });
});

// ── useBoardsCatalog ──────────────────────────────────────────────────────────

const MOCK_CATALOG: BoardCatalogEntry[] = [
  { id: 'greenhouse', displayName: 'Greenhouse', mode: 'http', auth: 'guest', listed: true },
  { id: 'linkedin', displayName: 'LinkedIn', mode: 'http', auth: 'optional', listed: true },
  { id: 'indeed', displayName: 'Indeed', mode: 'browser', auth: 'required', listed: true },
  { id: 'glassdoor', displayName: 'Glassdoor', mode: 'browser', auth: 'guest', listed: false },
];

describe('useBoardsCatalog', () => {
  it('returns the catalog from api.boards.catalog()', async () => {
    const client = createMockClient({
      'boards.catalog': vi.fn().mockResolvedValue(MOCK_CATALOG),
    });
    const { result } = renderHookWithClient(() => useBoardsCatalog(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(MOCK_CATALOG);
  });

  it('calls api.boards.catalog exactly once (staleTime=Infinity — no refetch)', async () => {
    const catalogFn = vi.fn().mockResolvedValue(MOCK_CATALOG);
    const client = createMockClient({ 'boards.catalog': catalogFn });
    const { result } = renderHookWithClient(() => useBoardsCatalog(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(catalogFn).toHaveBeenCalledTimes(1);
  });

  it('uses the [boards, catalog] query key', async () => {
    const { keys } = await import('@/services/query-client');
    expect(keys.boards.catalog).toEqual(['boards', 'catalog']);
  });

  it('returns an empty array when the backend returns []', async () => {
    const client = createMockClient({
      'boards.catalog': vi.fn().mockResolvedValue([]),
    });
    const { result } = renderHookWithClient(() => useBoardsCatalog(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual([]);
  });

  it('surfaces loading state before the query resolves', () => {
    // Never-resolving promise → hook stays in loading state
    const client = createMockClient({
      'boards.catalog': vi.fn(() => new Promise(() => {})),
    });
    const { result } = renderHookWithClient(() => useBoardsCatalog(), { client });

    // isLoading is true immediately (before any resolution)
    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeUndefined();
  });
});
