import { useCallback } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import type { HelpSearchRequest } from '@ajh/shared/schemas';

import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';

/**
 * Rank the help corpus against one user question (`help:search`).
 *
 * A mutation, not a query, and deliberately without a query key: every call
 * carries a different question, the reply is only ever consumed once by the
 * turn that asked, and caching it would mean holding the user's typed
 * questions in the React Query cache for the rest of the session. Same shape
 * as `useExportDiagnostics` — a one-shot backend action, not shared state.
 */
export const useHelpSearch = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (req: HelpSearchRequest) => api.help.search(req) });
};

/**
 * Fetch the four read-only lists the help chat's data glance is built from, ON
 * DEMAND — never on mount.
 *
 * Why imperative rather than four `useQuery` hooks: the glance is only ever
 * used to build a prompt for a question the user actually asked, and it names
 * the user's own applications. Mounting the queries would issue four IPC reads
 * (and hold that data in the renderer) for someone who opened Help to read a
 * single entry. Gating those hooks on an `enabled` flag does not work either:
 * the flag flips during `send`, so the FIRST question would be prompted with an
 * all-zero glance — a confident lie about the user's data, which is worse than
 * no glance at all.
 *
 * `fetchQuery` uses the SAME query keys as {@link useEmbeddingStatus},
 * {@link useInteractions}, {@link useApplications} and {@link useAutopilots},
 * so a cached list from elsewhere in the app is reused and the result is
 * cached for them in turn — this is one more reader of those queries, not a
 * private fetch path.
 */
export const useFetchHelpDataSources = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useCallback(
    async () =>
      Promise.all([
        qc.fetchQuery({
          queryKey: keys.ai.embeddingStatus,
          queryFn: () => api.ai.embeddingStatus(),
        }),
        qc.fetchQuery({
          queryKey: keys.postings.interactions(undefined),
          queryFn: () => api.scrape.listInteractions({}),
        }),
        qc.fetchQuery({ queryKey: keys.applications.all, queryFn: () => api.applications.list() }),
        qc.fetchQuery({ queryKey: keys.autopilot.all, queryFn: () => api.autopilot.list() }),
      ]),
    [api, qc]
  );
};
