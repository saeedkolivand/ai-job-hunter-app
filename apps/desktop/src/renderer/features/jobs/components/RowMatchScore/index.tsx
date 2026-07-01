import { Info } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, HoverPopover } from '@ajh/ui';

import { useRowMatchScore } from '@/features/jobs/providers';
import { MatchBand } from '@/lib/match-band';

/**
 * Presentational per-row match score. Scores are fetched on-demand when the
 * user opens a job (`JobDetailPane` calls `scoreJob`). Rows that haven't been
 * opened yet show no badge — this is correct behaviour, not a loading state.
 *
 * Estimate framing is AMBIENT: the always-visible "est." micro-label adjacent
 * to the band makes the disclaimer present without interaction. The info trigger
 * opens the full guidance sentence on hover/focus; touch users see the "est."
 * label regardless (non-blocking touch gap per reviewer agreement).
 */
export function RowMatchScore({ jobId }: { jobId: string }) {
  const { t } = useTranslation();
  const { score, hasResume } = useRowMatchScore(jobId);

  if (!hasResume) return null;
  if (!score) return null;

  return (
    <HoverPopover
      placement="top"
      ariaLabel={t('jobs.scoreGuidance')}
      trigger={
        <div className="flex items-center gap-1">
          <MatchBand value={score.combined} />
          {/* Always-visible estimate framing — present without interaction. */}
          <span className="text-fine-print text-foreground/50">{t('jobs.scoreEst')}</span>
          <Button
            variant="unstyled"
            className="flex h-6 w-6 items-center justify-center text-foreground/50 hover:text-foreground/70 focus-visible:text-foreground/70"
            aria-label={t('jobs.scoreGuidanceLabel')}
          >
            <Info size={14} aria-hidden="true" />
          </Button>
        </div>
      }
    >
      <p className="dropdown-surface max-w-[220px] px-3 py-2 text-fine-print leading-snug text-foreground/70">
        {t('jobs.scoreGuidance')}
      </p>
    </HoverPopover>
  );
}
