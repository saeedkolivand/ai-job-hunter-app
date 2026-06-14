import { useTranslation } from '@ajh/translations';

import { MatchBand } from '@/features/jobs/lib/score';
import { useRowMatchScore } from '@/features/jobs/providers';

/**
 * Presentational per-row match score. The combined keyword/semantic score is
 * supplied by MatchScoresProvider (one batch call for all filtered postings),
 * so this component holds no scheduling or fetching logic.
 */
export function RowMatchScore({ jobId }: { jobId: string }) {
  const { t } = useTranslation();
  const { score, pending, hasResume } = useRowMatchScore(jobId);

  if (!hasResume) return null;
  if (score) return <MatchBand value={score.combined} />;
  if (pending) {
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
  return null;
}
