import type { MatchScore } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

import { useSelectedProvider } from '@/components/ui/ModelSelector';
import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { MatchBand } from '@/lib/match-band';
import type { AiProvider } from '@/store/preferences-schema';

/**
 * Whether `score` is an actually-usable `MatchScore` — `invoke()` resolves
 * whatever the backend returns without validating its shape, so a failure
 * response (e.g. `{ error: "resume not found: …" }`, reachable when a
 * wizard/generation session's résumé id outlives the résumé it points at)
 * arrives here typed as `MatchScore` while `ats`/`combined`/`gaps`/
 * `recommendations` are actually `undefined`. Guarantees every field a caller
 * reads without its own null-check (`hasScoreCoverage`'s `score.gaps.length`,
 * `GenerationScoreStrip`'s `score.gaps.slice`/`score.recommendations.length`)
 * is actually present and typed correctly — without it, `Math.round(undefined)`
 * is `NaN` (lands on a red "Low" badge) or `.length` on a missing array throws
 * mid-render and unmounts the subtree instead of showing "not scored".
 */
export function isMeasured(score: MatchScore | undefined): score is MatchScore {
  return (
    !!score &&
    Number.isFinite(score.ats) &&
    Number.isFinite(score.combined) &&
    Array.isArray(score.gaps) &&
    Array.isArray(score.recommendations)
  );
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
 *  same `MatchScore`. `isMeasured` blocks a non-finite `value` at both
 *  current call sites, but a non-finite value is treated the same as `null`
 *  here too — a future caller skipping that guard must still get the honest
 *  placeholder, not a `NaN%` under a fabricated lowest-tier badge. */
export function ScoreMetric({ label, value, variant, notScoredLabel, testId }: ScoreMetricProps) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2">
      <span className="text-[11px] text-foreground/60">{label}</span>
      <span data-testid={testId} className="flex items-center gap-1.5">
        {value !== null && Number.isFinite(value) ? (
          <>
            <span className="text-[11px] font-semibold tabular-nums text-foreground/80">
              {Math.round(value)}%
            </span>
            {variant && <MatchBand value={value} variant={variant} />}
          </>
        ) : (
          <span className="text-[11px] font-medium text-foreground/70">{notScoredLabel}</span>
        )}
      </span>
    </div>
  );
}

/**
 * CLI-agent egress disclosure, shared by `JobAdView`'s Score tab and the
 * résumé result's score strip (`GenerationScoreStrip`) — both fire the same
 * `match:text` call, so both need to say so when it can actually leave the
 * machine. A CLI-agent provider (Claude Code, Codex, Gemini CLI) reads as
 * "local" everywhere else in the app, but a foreign-language posting routes
 * through translation, which egresses the posting text to it. Callers render
 * the result ONLY on branches reachable after a request went out (loading,
 * error, measured) — never on a state that short-circuits before the query
 * enables (no résumé stored, no posting text).
 */
export function useCliAgentEgressNotice() {
  const { t } = useTranslation();
  const activeProvider = useSelectedProvider();
  const activeProviderMeta = PROVIDERS[activeProvider as AiProvider];
  const isCliAgentProvider = activeProviderMeta?.kind === 'cli-agent';

  if (!isCliAgentProvider) return null;
  return (
    <p className="shrink-0 text-[10px] leading-relaxed text-foreground/70">
      {t('autopilot.apply.jobAdView.score.cliAgentEgress', {
        provider: activeProviderMeta?.label ?? activeProvider,
      })}
    </p>
  );
}
