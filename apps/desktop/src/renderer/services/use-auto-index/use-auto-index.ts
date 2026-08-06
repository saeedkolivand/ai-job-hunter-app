import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { JobEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { useAutoIndexOnUpload } from '@/store/preferences-store';

import { keys } from '../query-client';
import { useEmbeddingStatus } from '../use-ai-provider';
import { useJobEvents } from '../use-jobs';

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

  // The concurrency guard, held until the JOB ends — not until
  // `indexStaleDocuments` resolves, which happens as soon as the job is SPAWNED.
  // The status refetch right after that still reports the pre-run stale count,
  // and would otherwise start a second, separately-billed run over the same
  // documents.
  //
  // STATE rather than a ref because the effect must re-run when a job finishes,
  // and clearing a ref re-renders nothing.
  const [inFlightJob, setInFlightJob] = useState<string | null>(null);
  const starting = useRef(false);

  // One attempt per (space, stale count), CLEARED once the index goes clean or
  // the space changes. Both halves are load-bearing:
  //   - without the key, a run that fails to reduce `stale` re-triggers the
  //     moment it ends, looping paid provider calls forever;
  //   - without the reset, a later batch that happens to share a previous
  //     count is skipped forever — import one résumé (stale 1, indexed), import
  //     another (stale 1 again) and auto-indexing silently dies after the first.
  const attemptedFor = useRef<string | null>(null);

  // Release the guard when our job reaches a terminal state, then refresh the
  // strip so it shows the post-run count.
  useJobEvents((evt: JobEvent) => {
    const e = evt as { type: string; jobId: string };
    if (e.jobId !== inFlightJob) return;
    if (e.type !== 'job.completed' && e.type !== 'job.failed' && e.type !== 'job.cancelled') return;
    setInFlightJob(null);
    void qc.invalidateQueries({ queryKey: keys.ai.embeddingStatus });
  });

  useEffect(() => {
    const space = `${activeProvider}/${activeModel}`;
    // A clean index (or a space change) means any future non-zero count is new
    // work, not the run we already attempted.
    if (stale === 0 || attemptedFor.current?.startsWith(`${space}/`) === false) {
      attemptedFor.current = null;
    }

    if (!enabled || stale === 0) return;
    // An embedding space with no model configured cannot index anything; wait
    // for the user to pick one rather than failing once per refetch. This is the
    // normal first run — the résumé step precedes the AI step in onboarding.
    if (!activeProvider || !activeModel) return;
    if (inFlightJob || starting.current) return;

    const attempt = `${space}/${stale}`;
    if (attemptedFor.current === attempt) return;
    attemptedFor.current = attempt;

    starting.current = true;
    void (async () => {
      try {
        const { jobId } = await api.ai.indexStaleDocuments();
        // Null means nothing was actually stale by the time the backend looked
        // (a lazy embed during matching got there first) — no job, nothing to
        // wait for, and the next real change is free to trigger a run.
        if (jobId) setInFlightJob(jobId);
        else await qc.invalidateQueries({ queryKey: keys.ai.embeddingStatus });
      } catch {
        // Best-effort by design: matching still embeds lazily when it needs a
        // vector, so a failed pre-warm degrades to the old behaviour rather than
        // breaking anything. The provider's own message is deliberately not
        // logged — this line is persisted into diagnostics bundles.
        console.warn('[auto-index] stale-document indexing failed', {
          provider: activeProvider,
          stale,
        });
      } finally {
        starting.current = false;
      }
    })();
  }, [api, qc, enabled, stale, activeProvider, activeModel, inFlightJob]);
};
