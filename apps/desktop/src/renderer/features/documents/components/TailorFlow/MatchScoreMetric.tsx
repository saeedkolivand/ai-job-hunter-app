import type { MatchScore } from '@ajh/shared';

import { MatchBand } from '@/lib/match-band';

/**
 * Whether `score` is an actually-usable `MatchScore` — `invoke()` resolves
 * whatever the backend returns without validating its shape, so a failure
 * response (e.g. `{ error: "resume not found: …" }`, reachable when a
 * wizard/generation session's résumé id outlives the résumé it points at)
 * arrives here typed as `MatchScore` while `ats`/`combined` are actually
 * `undefined`. Without this guard, `Math.round(undefined)` is `NaN` and
 * `scoreTier(NaN)` fails every `>=` comparison and lands on a red "Low"
 * badge for a request that never scored anything.
 */
export function isMeasured(score: MatchScore | undefined): score is MatchScore {
  return !!score && Number.isFinite(score.ats) && Number.isFinite(score.combined);
}

/**
 * `keyword_coverage` (`match_resume.rs`) returns `ats: 0, gaps: []` for BOTH
 * "no extractable keywords" AND would for a genuine 0% match — except a
 * genuine 0% still lists every job keyword as a gap, so `gaps.length === 0`
 * only co-occurs with `ats === 0` in the former case. `combined` inherits the
 * same placeholder (its formula always needs a real `ats` input), so this
 * gates any Match/coverage-derived row too — never render a fake `0`.
 */
export function hasScoreCoverage(score: MatchScore | undefined): score is MatchScore {
  return isMeasured(score) && !(score.ats === 0 && score.gaps.length === 0);
}

export interface ScoreMetricProps {
  label: string;
  /** `null` for "not actually measured" — renders `notScoredLabel` instead of
   *  a fabricated `0`. Never pass a real `0` measurement as `null`. */
  value: number | null;
  variant?: 'combined' | 'coverage';
  notScoredLabel: string;
  testId: string;
}

/** One score row: a real percentage + tier badge, or an honest "not scored"
 *  placeholder — see the caller for what makes a value real vs. a
 *  placeholder. Shared by `JobAdView`'s Score tab and the résumé result's
 *  score strip so the two surfaces never drift into two renderings of the
 *  same `MatchScore`. */
export function ScoreMetric({ label, value, variant, notScoredLabel, testId }: ScoreMetricProps) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2">
      <span className="text-[11px] text-foreground/60">{label}</span>
      <span data-testid={testId} className="flex items-center gap-1.5">
        {value === null ? (
          <span className="text-[11px] font-medium text-foreground/70">{notScoredLabel}</span>
        ) : (
          <>
            <span className="text-[11px] font-semibold tabular-nums text-foreground/80">
              {Math.round(value)}%
            </span>
            {variant && <MatchBand value={value} variant={variant} />}
          </>
        )}
      </span>
    </div>
  );
}
