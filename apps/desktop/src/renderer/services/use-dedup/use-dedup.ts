import { useMutation, useQueryClient } from '@tanstack/react-query';

import type { DedupMarkNotDuplicateRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Split a wrongly-merged cross-board cluster (ADR-029): mark a member as NOT a
 * duplicate of the given other members. On success, invalidates the live
 * postings list and the autopilot queries so the now-ungrouped rows re-render
 * with the recomputed cluster annotations from the backend.
 */
export const useMarkNotDuplicate = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: DedupMarkNotDuplicateRequest) => api.dedup.markNotDuplicate(req),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.postings.all });
      void qc.invalidateQueries({ queryKey: keys.autopilot.all });
    },
  });
};
