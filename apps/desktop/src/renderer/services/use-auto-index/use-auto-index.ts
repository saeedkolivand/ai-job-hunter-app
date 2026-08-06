import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';
import { useAutoIndexOnUpload } from '@/store/preferences-store';

import { keys } from '../query-client';
import { useEmbeddingStatus } from '../use-ai-provider';

/**
 * Keep the embedding index up to date on its own, when the user has asked for it
 * (`autoIndexOnUpload`).
 *
 * Why this exists: a résumé's vector is read by exactly one thing — semantic
 * match scoring — and `match_resume` already embeds lazily when it finds no
 * usable vector. So indexing was never REQUIRED; the Settings button is a
 * pre-warm. What it cost was a slow first match (a synchronous embed inside
 * scoring) and a status strip that looked like a chore list.
 *
 * One effect covers all three moments that create stale documents, because they
 * all show up the same way — as a non-zero `documents.stale` from
 * `ai_embedding_status`:
 *   - a résumé was just imported (the documents query invalidates, status refetches)
 *   - the embedding provider/model changed (every vector is now in the wrong space)
 *   - the app started with either of the above left over from last session
 *
 * `indexStaleDocuments` embeds ONLY what is missing, and returns a null job id
 * when nothing is — so the common case is one cheap query and no work. It is
 * deliberately not `reembedAll`, which would re-bill a cloud embedding provider
 * for already-indexed documents every time a single file is added.
 */
export const useAutoIndex = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const enabled = useAutoIndexOnUpload();
  const { data: status } = useEmbeddingStatus();

  const stale = status?.documents?.stale ?? 0;
  const activeModel = status?.active?.model ?? '';
  const activeProvider = status?.active?.provider ?? '';

  // One run per (space, stale-count) situation. Without this the effect would
  // re-fire on every refetch while a run is still in flight and queue duplicate
  // index jobs for the same documents — paid duplicates, on a cloud provider.
  const inFlight = useRef(false);
  const lastAttempt = useRef('');

  useEffect(() => {
    if (!enabled || stale === 0) return;
    // An embedding space with no model configured cannot index anything; wait
    // for the user to pick one rather than failing once per refetch.
    if (!activeProvider || !activeModel) return;

    const attempt = `${activeProvider}/${activeModel}/${stale}`;
    if (inFlight.current || lastAttempt.current === attempt) return;
    inFlight.current = true;
    lastAttempt.current = attempt;

    void (async () => {
      try {
        await api.ai.indexStaleDocuments();
        // Refresh the strip so it reflects the new count rather than the one
        // that triggered this run.
        await qc.invalidateQueries({ queryKey: keys.ai.embeddingStatus });
      } catch (err) {
        // Best-effort by design: matching still embeds lazily when it needs a
        // vector, so a failed pre-warm degrades to the old behaviour rather
        // than breaking anything. Logged (never the raw provider message — see
        // `errorClass`) so a diagnostics bundle still shows it was attempted.
        console.warn('[auto-index] stale-document indexing failed', {
          provider: activeProvider,
          stale,
        });
        void err;
      } finally {
        inFlight.current = false;
      }
    })();
  }, [api, qc, enabled, stale, activeProvider, activeModel]);
};
