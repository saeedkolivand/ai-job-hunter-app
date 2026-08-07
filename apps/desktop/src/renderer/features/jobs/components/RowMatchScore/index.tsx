import { Info } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, HoverPopover } from '@ajh/ui';

import { useRowMatchScore } from '@/features/jobs/providers';
import { MatchBand, matchBandDescriptionKey, scoreTier } from '@/lib/match-band';

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

  // What this tier means, then the estimate disclaimer. The band itself is
  // `describe={false}` here — its native `title` would otherwise fire on hover
  // alongside this popover and say a shorter version of the same thing.
  const tierDescription = t(matchBandDescriptionKey(scoreTier(score.combined).key));

  return (
    <HoverPopover
      placement="top"
      ariaLabel={`${tierDescription} ${t('jobs.scoreGuidance')}`}
      trigger={
        <div className="flex items-center gap-1">
          <MatchBand value={score.combined} describe={false} />
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
      <div className="dropdown-surface max-w-[240px] space-y-1.5 px-3 py-2 text-fine-print leading-snug text-foreground/70">
        <p className="text-foreground/85">{tierDescription}</p>
        <p>{t('jobs.scoreGuidance')}</p>
      </div>
    </HoverPopover>
  );
}
