import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import type { InsightsOutcomesReport } from '@ajh/shared';

import { createMockClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useInsightsOutcomes } from './use-insights';

const REPORT: InsightsOutcomesReport = {
  groups: [
    {
      dimension: 'board',
      key: 'indeed',
      counts: { positiveReply: 1, rejected: 0, noReply: 2, pending: 0 },
      rates: null,
    },
  ],
  nudge: null,
  generatedAt: 1_794_000_000_000,
};

describe('useInsightsOutcomes', () => {
  it('returns the report from the client and caches it under its key', async () => {
    const outcomes = vi.fn().mockResolvedValue(REPORT);
    const client = createMockClient({ 'insights.outcomes': outcomes });
    const { result, queryClient } = renderHookWithClient(() => useInsightsOutcomes(), { client });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(REPORT);
    expect(outcomes).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryData(keys.insights.outcomes)).toEqual(REPORT);
  });
});
