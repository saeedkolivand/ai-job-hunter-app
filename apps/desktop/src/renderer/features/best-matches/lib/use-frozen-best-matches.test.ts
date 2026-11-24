import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { AutopilotBestMatch, AutopilotBestMatchesResult } from '@ajh/shared';

let mockData: AutopilotBestMatchesResult | undefined;
const mockRefetch = vi.fn();
vi.mock('@/services', () => ({
  useBestMatches: () => ({ data: mockData, refetch: mockRefetch }),
}));

import { useFrozenBestMatches } from './use-frozen-best-matches';

const row = (key: string): AutopilotBestMatch => ({
  key,
  title: key,
  company: 'Acme',
  url: `https://example.com/${key}`,
  score: 80,
  scoreSource: 'combined',
  foundAt: 0,
  sources: [],
});
const result = (...keys: string[]): AutopilotBestMatchesResult => ({
  matches: keys.map(row),
  total: 357,
  autopilotCount: 1,
});

describe('useFrozenBestMatches', () => {
  beforeEach(() => {
    mockRefetch.mockReset();
  });

  it('does not refill the displayed batch when a refetch brings different rows', () => {
    mockData = result('a', 'b');
    const { result: hook, rerender } = renderHook(() => useFrozenBestMatches());
    expect(hook.current.data?.matches.map((m) => m.key)).toEqual(['a', 'b']);

    // 'a' was dismissed server-side, so the live pool now offers 'c' in its place.
    mockData = result('b', 'c');
    rerender();

    expect(hook.current.data?.matches.map((m) => m.key)).toEqual(['a', 'b']);
  });

  it('replaces the batch only on an explicit refresh', async () => {
    mockData = result('a', 'b');
    const { result: hook } = renderHook(() => useFrozenBestMatches());
    mockRefetch.mockResolvedValue({ data: result('b', 'c') });

    await act(async () => {
      await hook.current.refresh();
    });

    expect(hook.current.data?.matches.map((m) => m.key)).toEqual(['b', 'c']);
  });

  it('keeps following live data while the first snapshot is empty', () => {
    mockData = result();
    const { result: hook, rerender } = renderHook(() => useFrozenBestMatches());

    mockData = result('a');
    rerender();

    expect(hook.current.data?.matches.map((m) => m.key)).toEqual(['a']);
  });
});
