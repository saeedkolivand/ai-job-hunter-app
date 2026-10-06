import { useEffect, useRef, useState } from 'react';

import { AGGREGATOR_BOARD_ID } from '@ajh/shared';

import { useMatchScores } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';
import { useResolveJobUrl, useUpdatePostingDescription } from '@/services';

// ponytail: heuristic threshold — Adzuna search snippets are ~200–500 chars;
// anything under 700 chars for aggregator postings gets an on-demand resolve.
const SHORT_DESCRIPTION_CHARS = 700;

/**
 * On-demand description resolution for the detail pane: decides whether the
 * snippet needs resolving, keeps the longer text, persists-then-scores once,
 * and flips `announced` when the snippet upgrades to the full text.
 */
export function useResolvedDescription(posting: Posting) {
  // On-demand resolve gate:
  //   1. Always resolve when description is empty (original behaviour).
  //   2. Also resolve for aggregator (Adzuna) postings whose description is a
  //      short snippet below the threshold — the full text lives on the redirect URL.
  const descriptionEmpty = !posting.description?.trim();
  const snippetLen = posting.description?.trim().length ?? 0;
  const isAggregatorShort =
    posting.source === AGGREGATOR_BOARD_ID && snippetLen < SHORT_DESCRIPTION_CHARS;
  const shouldResolve = descriptionEmpty || isAggregatorShort;

  const resolved = useResolveJobUrl(posting.url, shouldResolve);

  // Keep-longer merge: never render text shorter than the original snippet.
  // The resolved text wins only when it is meaningfully longer (guards against
  // a 429 / generic-HTML result degrading the pane).
  const resolvedText = resolved.data?.description ?? '';
  const description: string = (() => {
    if (descriptionEmpty) return resolvedText;
    if (resolvedText.length > snippetLen) return resolvedText;
    return posting.description ?? '';
  })();

  // Gate both the loading indicator and the retry button off isFetching so any
  // refetch (including the manual retry click) consistently drives the UI.
  const descLoading = shouldResolve && resolved.isFetching;

  // Show the retry button when the description may still be incomplete:
  //  - gate fired (aggregator-short or empty), AND
  //  - not currently fetching, AND
  //  - resolved text is not yet meaningfully longer than the snippet.
  const resolvedLonger = resolvedText.length > snippetLen;
  const showLoadButton =
    (isAggregatorShort || descriptionEmpty) && !resolved.isFetching && !resolvedLonger;

  // Persist-then-score: one ordered one-shot effect so the backend always reads
  // the full markdown when computing the match score.
  //
  // Race that this fixes: on the render where resolve settles with longer text,
  // two independent effects could fire in the same commit — scoreJob's match.resume
  // might hit the backend BEFORE updateDescription persisted the full text, so the
  // score would be computed on the stale snippet while the pane shows the full text.
  //
  // Fix: single effect, single `doneRef` guard. When the resolve produced longer
  // text, persist FIRST then score inside `.finally()` (persist failure is non-fatal).
  // When no persist is needed (already-full description or resolve didn't improve),
  // score immediately. key={posting.id} on the parent resets the ref per job.
  const { scoreJob } = useMatchScores();
  const { mutateAsync: updateDescription } = useUpdatePostingDescription();
  const doneRef = useRef(false);
  useEffect(() => {
    if (doneRef.current) return;
    const descReady = description.trim().length > 0;
    // isFetched guards against the window before the query has started fetching —
    // without it resolveSettled could be true on the first render (isFetching=false,
    // data=undefined) and we'd score the snippet before the resolve even begins.
    const resolveSettled =
      !shouldResolve || (resolved.isFetched && !resolved.isFetching && !descLoading);
    if (!descReady || !resolveSettled) return;
    if (resolvedLonger) {
      // Persist the full text first, then score on it.
      // Latch only AFTER persist resolves so a transient IPC failure doesn't
      // permanently prevent a re-score with the full text.
      doneRef.current = true;
      void updateDescription({ url: posting.url, description })
        .catch(() => {
          // Persist failure is non-fatal — still score off the in-memory text,
          // but clear the latch so the pane can retry on next open.
          doneRef.current = false;
        })
        .then(() => scoreJob(posting.id));
    } else {
      // No persist needed — score immediately.
      doneRef.current = true;
      scoreJob(posting.id);
    }
  }, [
    description,
    descLoading,
    posting.id,
    posting.url,
    resolved.isFetched,
    resolved.isFetching,
    resolvedLonger,
    scoreJob,
    shouldResolve,
    updateDescription,
  ]);

  // Polite AT announcement when the description upgrades to the full text.
  const [announced, setAnnounced] = useState(false);
  const prevDescLen = useRef(description.length);
  useEffect(() => {
    if (!announced && description.length > prevDescLen.current && prevDescLen.current > 0) {
      setAnnounced(true);
    }
    prevDescLen.current = description.length;
  }, [announced, description.length]);

  return {
    description,
    descLoading,
    showLoadButton,
    showError: shouldResolve && resolved.isError && !descLoading,
    refetch: resolved.refetch,
    announced,
  };
}
