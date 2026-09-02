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
 *
 * Each source resolves INDEPENDENTLY: the glance is an optional garnish on an
 * answer that is really grounded in the help corpus, so one unreadable source
 * must cost its own line and nothing else. Under `Promise.all` a single
 * rejection — a locked database, an embedding backend that is down — threw the
 * whole question away and the user got an error instead of an answer that was
 * always going to come from the corpus anyway.
 */
export const useFetchHelpDataSources = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useCallback(async () => {
    const [embeddingStatus, interactions, applications, autopilots] = await Promise.allSettled([
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
    ]);
    return [
      unavailableAsNull(embeddingStatus),
      unavailableAsNull(interactions),
      unavailableAsNull(applications),
      unavailableAsNull(autopilots),
    ] as const;
  }, [api, qc]);
};

/**
 * One source's outcome, with a failure reported as `null` — UNKNOWN, not zero.
 *
 * The distinction is the whole point. `buildHelpDataGlance` states its numbers
 * as fact, so degrading a failed read to `0`/`[]` would put "Documents
 * imported: 0" in front of the model for a user with fifty documents, and the
 * answer would confidently act on it. `null` means the glance omits the line,
 * which is the same reason the glance is not fetched on mount: about the
 * user's own data, saying nothing beats saying something false.
 */
function unavailableAsNull<T>(result: PromiseSettledResult<T>): T | null {
  return result.status === 'fulfilled' ? result.value : null;
}
