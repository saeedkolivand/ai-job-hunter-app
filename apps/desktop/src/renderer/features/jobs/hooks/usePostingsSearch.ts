import { useCallback, useRef, useState } from 'react';

import type { HybridSearchResult } from '@ajh/shared';

import { useMachine } from '@/hooks/use-machine';
import { postingsSearchMachine } from '@/lib/machines/postings-search.machine';
import { useCancelJob, useHybridSearch, useSetSemanticScoring } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

export type { PostingsSearchState } from '@/lib/machines/postings-search.machine';

/**
 * Wires the real UX onto the minimal `useHybridSearch` mutation (see its
 * doc): mints a fresh `queryId` per search, cancels the previous in-flight
 * search before firing the next one (a Tauri invoke isn't abortable from the
 * renderer — `jobs.cancel` is the only way to stop the backend embedding/
 * reranking a result nobody will see), and discards a response that settles
 * after a newer search has already been issued (out-of-order IPC
 * resolution) or that reports `outcome: 'cancelled'` — by construction that
 * only happens to a search WE superseded, so it is never surfaced, which is
 * what makes it distinct from a genuine error.
 *
 * `committedQuery` is exposed so a caller (`JobsPage`) can tell whether the
 * search still matches what's currently typed in the filter box: editing the
 * text after a search has settled should fall back to instant substring
 * filtering rather than keep showing a stale ranked list under new text.
 */
export function usePostingsSearch() {
  const hybridSearch = useHybridSearch();
  const cancelJob = useCancelJob();
  const syncSemanticScoring = useSetSemanticScoring();
  const [state, send] = useMachine(postingsSearchMachine, 'idle');
  const [result, setResult] = useState<HybridSearchResult | null>(null);
  const [committedQuery, setCommittedQuery] = useState('');
  const latestQueryIdRef = useRef<string | null>(null);
  const lastQueryRef = useRef('');

  const search = useCallback(
    (query: string, eligibleIds: string[]) => {
      const trimmed = query.trim();
      if (!trimmed) return;
      const previousQueryId = latestQueryIdRef.current;
      const queryId = crypto.randomUUID();
      latestQueryIdRef.current = queryId;
      lastQueryRef.current = trimmed;
      setCommittedQuery(trimmed);
      send('SUBMIT');
      // Best-effort, fire-and-forget: the superseded search keeps
      // embedding/reranking in Rust either way (the invoke promise isn't
      // abortable) — this only stops it sooner. Never blocks the new search.
      if (previousQueryId) void cancelJob.mutateAsync(previousQueryId).catch(() => {});
      hybridSearch.mutate(
        { queryId, query: trimmed, eligibleIds, limit: 20 },
        {
          onSuccess: (data, variables) => {
            if (variables.queryId !== latestQueryIdRef.current) return; // superseded
            if (data.outcome === 'cancelled') return; // superseded by design — never surfaced
            setResult(data);
            if (data.outcome === 'staleCorpus') send('SETTLED_STALE');
            else if (data.hits.length === 0) send('SETTLED_EMPTY');
            else send('SETTLED_RESULTS');
          },
          onError: (_err, variables) => {
            if (variables.queryId !== latestQueryIdRef.current) return; // superseded
            send('FAILED');
          },
        }
      );
    },
    [hybridSearch, cancelJob, send]
  );

  /** Re-issue the last committed query — used by the stale/error retry action
   *  and by {@link enableSemanticRanking} once the preference flips. */
  const retry = useCallback(
    (eligibleIds: string[]) => {
      if (!lastQueryRef.current) return;
      search(lastQueryRef.current, eligibleIds);
    },
    [search]
  );

  /** Dismiss the active search (e.g. "Clear search") — reverts the caller to
   *  instant substring filtering without touching the typed text itself. */
  const clear = useCallback(() => {
    const previousQueryId = latestQueryIdRef.current;
    latestQueryIdRef.current = null;
    lastQueryRef.current = '';
    setCommittedQuery('');
    setResult(null);
    send('CLEAR');
    if (previousQueryId) void cancelJob.mutateAsync(previousQueryId).catch(() => {});
  }, [cancelJob, send]);

  /**
   * One-click remediation for the most common degraded case
   * (`arms.dense === 'skipped'`, `semanticScoring` defaults OFF): flips the
   * preference, mirrors it to the backend-readable copy the headless
   * Autopilot scheduler reads (the same write-through `EmbeddingsSettings`
   * uses), and re-runs the last search so the user sees the improved ranking
   * immediately instead of pressing Search again.
   */
  const enableSemanticRanking = useCallback(
    (eligibleIds: string[]) => {
      usePreferencesStore.getState().setSemanticScoring(true);
      syncSemanticScoring.mutate(true);
      retry(eligibleIds);
    },
    [retry, syncSemanticScoring]
  );

  return { state, result, committedQuery, search, retry, clear, enableSemanticRanking };
}
