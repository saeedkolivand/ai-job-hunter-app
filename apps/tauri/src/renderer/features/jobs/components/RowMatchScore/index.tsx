import { useEffect } from 'react';

import type { MatchScore } from '@ajh/shared';

import { MatchBand } from '@/features/jobs/lib/score';
import { useTranslation } from '@/lib/i18n';
import { useScoringScheduler } from '@/providers/ScoringScheduler';
import { useDocuments, useJobMatchScore } from '@/services';

interface RawDoc {
  _id: string;
  isDefault?: boolean;
}

/** Resolve the default saved résumé's id (`_id`), or the first saved one. */
function useDefaultResumeId(): string | null {
  const { data = [] } = useDocuments();
  const docs = data as unknown as RawDoc[];
  const def = docs.find((d) => d.isDefault) ?? docs[0];
  return def?._id ?? null;
}

/**
 * Per-row match score (#50). Enqueues this posting in the FIFO
 * ScoringScheduler so only one embedding call is active at a time.
 * Once results arrive the slot is released and the next queued row runs.
 * Results are cached for 10 minutes so navigating the list never re-fires.
 */
export function RowMatchScore({ jobId }: { jobId: string }) {
  const { t } = useTranslation();
  const resumeId = useDefaultResumeId();

  const { activeSet, enqueue, release, remove } = useScoringScheduler();
  const enabled = !!resumeId && !!jobId && activeSet.has(jobId);

  useEffect(() => {
    if (!resumeId || !jobId) return;
    enqueue(jobId);
    return () => {
      remove(jobId);
    };
  }, [jobId, resumeId, enqueue, remove]);

  const { data, isPending, isError } = useJobMatchScore(resumeId, jobId, enabled);

  useEffect(() => {
    if (enabled && !isPending) release(jobId);
  }, [enabled, isPending, jobId, release]);

  // No resume selected or scoring failed — render nothing to keep the row clean.
  if (!resumeId || isError) return null;

  if (isPending) {
    return (
      <span
        className="text-[11px] text-foreground/30"
        aria-label={t('jobs.scoreLoading')}
        aria-busy="true"
      >
        …
      </span>
    );
  }

  const result = data as MatchScore & { error?: string };
  if (result.error) return null;

  return <MatchBand value={result.combined} />;
}
