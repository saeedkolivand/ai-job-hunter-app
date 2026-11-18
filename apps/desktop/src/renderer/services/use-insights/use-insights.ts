import { useQuery } from '@tanstack/react-query';

import type { InsightsOutcomesReport } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Outcome insights + the volume nudge, derived by the backend on every fetch.
 * Application writes go through `invalidateApplications`, which refreshes this key too;
 * the page also reads it fresh on mount (`staleTime: 0`).
 */
export const useInsightsOutcomes = () => {
  const api = useAppClient();
  return useQuery<InsightsOutcomesReport>({
    queryKey: keys.insights.outcomes,
    queryFn: () => api.insights.outcomes(),
    staleTime: 0,
    // A local IPC read: it must not pause while the machine is offline.
    networkMode: 'always',
  });
};
