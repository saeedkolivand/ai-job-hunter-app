import { Loader2, RefreshCw } from 'lucide-react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { useJobAdTextMatchScore } from '@/services';

import {
  hasScoreCoverage,
  isMeasured,
  ScoreMetric,
  useCliAgentEgressNotice,
} from './MatchScoreMetric';

interface GenerationScoreStripProps {
  /** Saved résumé backing this generation — the ORIGINAL, unedited résumé,
   *  same one `JobAdView`'s Score tab reads (see its own `resumeId` doc).
   *  Absent → the strip renders the "no résumé" reason, never a score. */
  resumeId?: string;
  /** The SNAPSHOTTED posting text, owned and re-snapshotted by `GenerationOutput`
   *  (never wire this to a live-editable `jobDesc`) — this component only
   *  renders on `view === 'doc' && activeOut === 'resume'`, so it unmounts on
   *  every tab switch; a snapshot living IN it would re-initialise from
   *  whatever `jobDesc` is live at remount, silently reintroducing the
   *  per-keystroke/translation-egress bug the snapshot exists to prevent (see
   *  `GenerationOutput`'s `scoreSnapshot`, and `JobAdView`'s `handleTabChange`
   *  for the sibling invariant on the Score tab). */
  jobDesc: string;
  className?: string;
}

/**
 * Compact score readout for the résumé result — one click closer than
 * `JobAdView`'s own Score sub-tab (Documents → Job ad → Score). Reuses
 * `useJobAdTextMatchScore`, the SAME `match:text` IPC and SAME content-
 * addressed query key `JobAdView` uses, so opening both never double-fetches.
 * Cover-letter output is not scored against keyword coverage, so the caller
 * only mounts this on the résumé tab.
 *
 * Every branch renders a single-row (or few-row), `shrink-0` shell — never
 * `EmptyState`/`ErrorState`'s full icon-boxed layout. A past bug in this
 * exact panel let an unbounded empty/loading state grow into a ~600px panel;
 * this strip has no `flex-1` anywhere in its own tree, so it can't repeat it.
 */
export function GenerationScoreStrip({ resumeId, jobDesc, className }: GenerationScoreStripProps) {
  const { t } = useTranslation();
  const egressNotice = useCliAgentEgressNotice();

  const scoreText = jobDesc.trim();
  const enabled = !!resumeId && !!scoreText;
  const {
    data: score,
    isLoading,
    isError,
    refetch,
  } = useJobAdTextMatchScore(resumeId ?? null, scoreText, enabled);

  const hasCoverage = hasScoreCoverage(score);

  const shellClassName = cn(
    'shrink-0 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2',
    className
  );

  if (!resumeId) {
    return (
      <div data-testid={TEST_IDS.documents.scoreStrip} className={shellClassName}>
        <p className="text-[11px] text-foreground/50">{t('jobs.scoreNoResume')}</p>
      </div>
    );
  }

  if (!scoreText) {
    return (
      <div data-testid={TEST_IDS.documents.scoreStrip} className={shellClassName}>
        <p className="text-[11px] text-foreground/50">
          {t('autopilot.apply.jobAdView.score.noPosting')}
        </p>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div
        data-testid={TEST_IDS.documents.scoreStrip}
        role="status"
        aria-live="polite"
        className={cn(shellClassName, 'flex flex-col gap-1.5')}
      >
        {egressNotice}
        <div className="flex items-center gap-2 text-[11px] text-foreground/40">
          <Loader2 size={12} className="animate-spin" />
          {t('autopilot.apply.jobAdView.score.loading')}
        </div>
      </div>
    );
  }

  if (isError || (score && !isMeasured(score))) {
    return (
      <div
        data-testid={TEST_IDS.documents.scoreStrip}
        role="alert"
        className={cn(shellClassName, 'flex flex-col gap-1.5 border-red-400/20 bg-red-400/5')}
      >
        {egressNotice}
        <div className="flex items-center justify-between gap-2">
          <span className="text-[11px] text-red-300/80">
            {t('autopilot.apply.jobAdView.score.errorTitle')}
          </span>
          <Button
            variant="ghost"
            onClick={() => void refetch()}
            className="flex h-auto items-center gap-1 px-1.5 py-0.5 text-[10px] text-foreground/60"
          >
            <RefreshCw size={10} />
            {t('autopilot.apply.tryAgain')}
          </Button>
        </div>
      </div>
    );
  }

  if (!score) {
    return (
      <div data-testid={TEST_IDS.documents.scoreStrip} className={shellClassName}>
        <p className="text-[11px] text-foreground/50">{t('analyze.notScored')}</p>
      </div>
    );
  }

  // More horizontal room here than the Score tab's narrow column, but still
  // capped — 6 keeps the chip row to one or two wraps instead of the full
  // list, which for a boilerplate-heavy posting can run past a hundred.
  const gaps = hasCoverage ? score.gaps.slice(0, 6) : [];

  return (
    <div
      data-testid={TEST_IDS.documents.scoreStrip}
      className={cn(shellClassName, 'flex flex-col gap-1.5')}
    >
      {egressNotice}
      <p className="text-[10px] leading-relaxed text-foreground/40">{t('jobs.scoreGuidance')}</p>
      <ScoreMetric
        label={t('autopilot.scoreAbbr.coverage')}
        value={hasCoverage ? score.ats : null}
        variant="coverage"
        notScoredLabel={t('autopilot.apply.jobAdView.score.noKeywords')}
        testId={TEST_IDS.documents.scoreStripCoverage}
      />
      {gaps.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-[10px] text-foreground/50">{t('analyze.gaps')}</span>
          <div className="flex flex-wrap gap-1">
            {gaps.map((gap) => (
              <span
                key={gap}
                className="rounded-full border border-amber-400/20 bg-amber-400/5 px-2 py-0.5 text-[10px] text-amber-300/90"
              >
                {gap}
              </span>
            ))}
          </div>
        </div>
      )}
      {/* Backend-authored English prose (`match_resume.rs`'s `recommendations`
          fn) — deliberately not run through `t()`, same precedent as
          `JobAdView`'s `explanationText`: there is nothing to translate a
          runtime-generated English sentence INTO. Gated on `hasCoverage` —
          the "no extractable keywords" placeholder still runs the same fn
          over its empty `gaps`, which would print a false "Strong keyword
          coverage" for a posting that was never actually scored. */}
      {hasCoverage && score.recommendations.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-[10px] text-foreground/50">{t('analyze.recommendations')}</span>
          {score.recommendations.map((rec) => (
            <p key={rec} className="text-[10px] leading-relaxed text-foreground/50">
              {rec}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
