import { useEffect, useRef, useState } from 'react';

import { AGGREGATOR_BOARD_ID, type UpdateDescriptionResult } from '@ajh/shared';

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
  const resolvedPosting = resolved.data;
  const resolvedText = resolvedPosting?.description ?? '';
  const resolvedLonger = resolvedText.length > snippetLen;
  const mergedText: string = (() => {
    if (descriptionEmpty) return resolvedText;
    if (resolvedLonger) return resolvedText;
    return posting.description ?? '';
  })();

  // Identity gate (#1445): a longer resolved text is only SHOWN once the backend
  // verdict said it is the same posting. Until then (and forever on drift) the
  // pane keeps the stored text, so the other advert never flashes.
  const [verdictOk, setVerdictOk] = useState(false);
  const [verdictFailed, setVerdictFailed] = useState(false);
  // 'unverified': a stored copy exists but nothing could be compared (or the drift
  // was dismissed) — nothing was written, so the resolved text stays hidden.
  const [unverified, setUnverified] = useState(false);
  const [drift, setDrift] = useState<{ title: string; company: string } | null>(null);
  const description: string =
    resolvedLonger && !verdictOk ? (posting.description ?? '') : mergedText;
  const verdictPending =
    resolvedLonger && resolved.isFetched && !verdictOk && !drift && !verdictFailed && !unverified;

  // Gate both the loading indicator and the retry button off isFetching so any
  // refetch (including the manual retry click) consistently drives the UI.
  const descLoading = shouldResolve && (resolved.isFetching || verdictPending);

  // Show the retry button when the description may still be incomplete:
  //  - gate fired (aggregator-short or empty), AND
  //  - not currently fetching, AND
  //  - resolved text is not yet meaningfully longer than the snippet.
  const showLoadButton =
    ((isAggregatorShort || descriptionEmpty) && !resolved.isFetching && !resolvedLonger) ||
    verdictFailed;

  // Persist-then-score: one ordered one-shot effect so the backend always reads
  // the full markdown when computing the match score, and the identity verdict
  // decides whether that text may be shown/scored at all.
  //
  // When the resolve produced longer text, persist FIRST; on 'identityDrift' the
  // text is discarded (no display, no score); otherwise reveal it and score.
  // Persist failure FAILS CLOSED (the backend rejects over-cap text and malformed
  // identities deterministically): the resolved text is never shown or scored,
  // the latch is cleared so the next open retries, and `verdictFailed` drives
  // the error/retry UI.
  // When no persist is needed, score immediately. key={posting.id} on the
  // parent resets the ref/state per job.
  const { scoreJob } = useMatchScores();
  const { mutateAsync: updateDescription } = useUpdatePostingDescription();
  const doneRef = useRef(false);
  useEffect(() => {
    if (doneRef.current || verdictFailed) return;
    const descReady = mergedText.trim().length > 0;
    // isFetched guards against the window before the query has started fetching —
    // without it resolveSettled could be true on the first render (isFetching=false,
    // data=undefined) and we'd score the snippet before the resolve even begins.
    const resolveSettled = !shouldResolve || (resolved.isFetched && !resolved.isFetching);
    if (!descReady || !resolveSettled) return;
    doneRef.current = true;
    if (!resolvedLonger) {
      // No persist needed — score immediately.
      scoreJob(posting.id);
      return;
    }
    const resolvedIdentity = {
      title: resolvedPosting?.title ?? posting.title,
      company: resolvedPosting?.company ?? posting.company,
      url: resolvedPosting?.url ?? posting.url,
      externalId: resolvedPosting?.externalId,
    };
    void (async () => {
      let result: UpdateDescriptionResult;
      try {
        result = await updateDescription({
          url: posting.url,
          description: mergedText,
          resolved: resolvedIdentity,
        });
      } catch {
        doneRef.current = false;
        setVerdictFailed(true);
        return;
      }
      if (result === 'identityDrift') {
        setDrift({ title: resolvedIdentity.title, company: resolvedIdentity.company });
        return;
      }
      if (result === 'unverified') {
        // Latch stays set: no re-persist on every render.
        setUnverified(true);
        return;
      }
      // 'updated' and 'noMatch' (no stored row or cache entry exists, so nothing
      // to protect) both reveal.
      setVerdictOk(true);
      scoreJob(posting.id);
    })();
  }, [
    mergedText,
    posting.id,
    posting.title,
    posting.company,
    posting.url,
    resolvedPosting,
    resolved.isFetched,
    resolved.isFetching,
    resolvedLonger,
    scoreJob,
    shouldResolve,
    updateDescription,
    verdictFailed,
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
    showError: shouldResolve && (resolved.isError || verdictFailed) && !descLoading,
    refetch: () => {
      setVerdictFailed(false);
      return resolved.refetch();
    },
    announced,
    drift,
    unverified,
  };
}
