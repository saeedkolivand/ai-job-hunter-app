import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared/ipc';
import { ReferralUpsertSchema } from '@ajh/shared/schemas';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * List the locally-stored referral contacts for one job (by `jobUrl`). Omitting
 * `jobUrl` lists all of them. Cache key is scoped per job so each Apply modal's
 * list stays isolated.
 */
export const useReferrals = (jobUrl?: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.referrals.list(jobUrl),
    queryFn: () => api.referrals.list(jobUrl),
    // Skip the round-trip for callers without a job URL (e.g. a card whose
    // generation has no jobUrl) instead of querying for an empty key.
    enabled: !!jobUrl,
  });
};

/**
 * Create or update a referral contact. Invalidates every referral list (the
 * `referrals` root key covers all per-job lists) so the new/edited row appears.
 */
export const useUpsertReferral = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    // Validate against the shared schema (default-filling + enum/shape checks)
    // before it crosses the IPC boundary.
    mutationFn: (req: ReferralUpsertRequest) =>
      api.referrals.upsert(ReferralUpsertSchema.parse(req)),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.referrals.all }),
  });
};

/**
 * Delete a referral contact. Optimistic — like {@link useRemoveAiGeneration}, the
 * row is dropped from every cached list immediately and restored if the backend
 * fails. Patches the lists across job scopes by filtering on id.
 */
export const useRemoveReferral = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.referrals.remove(id),
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: keys.referrals.all });
      const previous = qc.getQueriesData<ReferralContact[]>({ queryKey: keys.referrals.all });
      qc.setQueriesData<ReferralContact[]>({ queryKey: keys.referrals.all }, (old) =>
        (old ?? []).filter((r) => r.id !== id)
      );
      return { previous };
    },
    onError: (_err, _id, ctx) => {
      ctx?.previous?.forEach(([key, data]) => qc.setQueryData(key, data));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.referrals.all }),
  });
};
