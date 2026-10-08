import { useCallback } from 'react';

import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { transition } from '@/lib/machine';
import {
  type PostingsSearchEvent,
  postingsSearchMachine,
} from '@/lib/machines/postings-search.machine';
import { useCancelJob, useHybridSearch, useSetSemanticScoring } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';
import { type PostingsSearchSlice, useSessionStore } from '@/store/session-store';

export type { PostingsSearchState } from '@/lib/machines/postings-search.machine';

/**
 * Prefix every minted `queryId` carries. MUST match the Rust-side validation
 * in `commands::hybrid_search` (`scrape_hybrid_search`), which requires this
 * exact prefix on top of the `PostingsHybridSearchRequestSchema` length cap —
 * mirrored, not shared, the same way `QUERY_MAX_CHARS`/`ELIGIBLE_IDS_MAX`
 * there re-validate a Zod-schema cap rather than import it. A UUID v4 is 36
 * chars, so the prefixed id stays well under the 64-char cap.
 */
const QUERY_ID_PREFIX = 'search-';

/** Applies a machine event to the session-held search slice. */
function send(event: PostingsSearchEvent) {
  const { jobs, setJobs } = useSessionStore.getState();
  const state = transition(postingsSearchMachine, jobs.search.state, event);
  setJobs({ search: { ...jobs.search, state } });
}
function patch(p: Partial<PostingsSearchSlice>) {
  const { jobs, setJobs } = useSessionStore.getState();
  setJobs({ search: { ...jobs.search, ...p } });
}

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
  const { t } = useTranslation();
  const notify = useNotification();
  const hybridSearch = useHybridSearch();
  const cancelJob = useCancelJob();
  const syncSemanticScoring = useSetSemanticScoring();
  const { state, result, committedQuery } = useSessionStore((s) => s.jobs.search);

  const search = useCallback(
    (query: string, eligibleIds: string[]) => {
      const trimmed = query.trim();
      if (!trimmed) return;
      const previousQueryId = useSessionStore.getState().jobs.search.queryId;
      const queryId = `${QUERY_ID_PREFIX}${crypto.randomUUID()}`;
      patch({ queryId, committedQuery: trimmed });
      send('SUBMIT');
      // Best-effort, fire-and-forget: the superseded search keeps
      // embedding/reranking in Rust either way (the invoke promise isn't
      // abortable) — this only stops it sooner. Never blocks the new search.
      if (previousQueryId) void cancelJob.mutateAsync(previousQueryId).catch(() => {});
      // `mutateAsync` (not `mutate` callbacks): those are dropped when the page
      // unmounts, which would strand the stored state on 'searching'.
      const superseded = () => useSessionStore.getState().jobs.search.queryId !== queryId;
      hybridSearch
        .mutateAsync({ queryId, query: trimmed, eligibleIds, limit: 20 })
        .then((data) => {
          if (superseded()) return;
          if (data.outcome === 'cancelled') {
            // Cancelled backend-side while still the CURRENT search: nothing will
            // follow, so don't strand the state on 'searching'.
            patch({ queryId: null, committedQuery: '' });
            send('CLEAR');
            return;
          }
          patch({ result: data });
          if (data.outcome === 'staleCorpus') send('SETTLED_STALE');
          else if (data.hits.length === 0) send('SETTLED_EMPTY');
          else send('SETTLED_RESULTS');
        })
        .catch(() => {
          if (!superseded()) send('FAILED');
        });
    },
    [hybridSearch, cancelJob]
  );

  /** Re-issue the last committed query — used by the stale/error retry action
   *  and by {@link enableSemanticRanking} once the preference flips. */
  const retry = useCallback(
    (eligibleIds: string[]) => {
      const last = useSessionStore.getState().jobs.search.committedQuery;
      if (!last) return;
      search(last, eligibleIds);
    },
    [search]
  );

  /** Dismiss the active search (e.g. "Clear search") AND the typed filter text
   *  (the "Search: …" chip), so the full list is restored. */
  const clear = useCallback(() => {
    const previousQueryId = useSessionStore.getState().jobs.search.queryId;
    patch({ queryId: null, committedQuery: '', result: null });
    send('CLEAR');
    useSessionStore.getState().setJobs({ filter: '' });
    if (previousQueryId) void cancelJob.mutateAsync(previousQueryId).catch(() => {});
  }, [cancelJob]);

  /**
   * One-click remediation for the most common degraded case
   * (`arms.dense === 'skipped'`, `semanticScoring` defaults OFF): flips the
   * preference, mirrors it to the backend-readable copy the headless
   * Autopilot scheduler reads (the same write-through `EmbeddingsSettings`
   * uses, including its `onError` — a failed mirror write is not cosmetic:
   * in-app scoring would follow the Zustand value that just flipped while the
   * scheduler keeps reading the old one until the next successful write).
   *
   * The retry is sequenced INSIDE the sync mutation's `onSuccess`, never
   * fired alongside it: `scrape_hybrid_search` reads `semantic_scoring` from
   * the Rust-side store — the one THIS mutation writes — so retrying before
   * it resolves would race a still-stale backend value and come back with
   * the dense arm `skipped` again on the very click meant to fix that,
   * undercutting the CTA. On failure we do NOT retry — `onError`'s toast
   * already explains that nothing changed.
   */
  const enableSemanticRanking = useCallback(
    (eligibleIds: string[]) => {
      usePreferencesStore.getState().setSemanticScoring(true);
      syncSemanticScoring.mutate(true, {
        onSuccess: () => retry(eligibleIds),
        onError: () =>
          notify.error({ message: t('settings.embeddings.semanticScoringSyncFailed') }),
      });
    },
    [retry, syncSemanticScoring, notify, t]
  );

  return { state, result, committedQuery, search, retry, clear, enableSemanticRanking };
}
