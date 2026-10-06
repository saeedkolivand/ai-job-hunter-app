import { FileSearch, FileText, Loader2 } from 'lucide-react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { EmptyState, ErrorState } from '@ajh/ui';

import { useJobAdTextMatchScore } from '@/services';

import {
  hasScoreCoverage,
  isMeasured,
  ScoreMetric,
  useCliAgentEgressNotice,
} from '../MatchScoreMetric';

interface JobAdScorePanelProps {
  /** Whether the Score tab is showing; off-tab the hooks stay mounted but the panel renders null. */
  active: boolean;
  /** Saved résumé backing this generation — see `JobAdView`'s `resumeId`. */
  resumeId?: string;
  /** The posting text frozen when the Score tab opened (never the live text). */
  scoreSnapshot: string | null;
}

/**
 * Score tab — the "before" half of a comparison in the results panel,
 * a plain readout in the wizard. Every number here traces back to
 * ONE `MatchScore`, for the snapshot taken when this tab opened; an
 * absent input is a stated reason, never a `0`. Estimate framing
 * matches the Jobs page (`jobs.scoreGuidance`).
 */
export function JobAdScorePanel({ active, resumeId, scoreSnapshot }: JobAdScorePanelProps) {
  const { t } = useTranslation();

  // Score-tab egress disclosure — shared with the résumé result's score strip
  // (GenerationScoreStrip), see `useCliAgentEgressNotice`'s doc.
  const egressNotice = useCliAgentEgressNotice();

  // Lazy: only fires once a résumé is stored, AND the snapshot has real text —
  // a whitespace-only paste is still "empty" (the IPC schema only requires
  // length >= 1, so this guard is the real check).
  const scoreText = scoreSnapshot?.trim() ?? '';
  const scoreEnabled = active && !!resumeId && !!scoreText;
  const {
    data: score,
    isLoading: scoreLoading,
    isError: scoreError,
    refetch: refetchScore,
  } = useJobAdTextMatchScore(resumeId ?? null, scoreText, scoreEnabled);
  // See `hasScoreCoverage`'s doc (shared with the résumé result's score
  // strip) — `ats: 0, gaps: []` is the "no extractable keywords" placeholder,
  // never a fake `0`.
  const hasCoverage = hasScoreCoverage(score);
  // `match:text` (the IPC command behind `useJobAdTextMatchScore`) is gated on
  // the SAME `semanticScoring` app preference the Jobs page reads — the hook
  // threads it through automatically. `scoreSource` is `'combined'` only when
  // that preference is ON *and* a real embedding pair backed the comparison;
  // it stays `'keyword'` both when the preference is off and when it's on but
  // the embed degraded (offline provider, failed round-trip — see
  // `score_one`'s doc). Either `'keyword'` case renders the honest
  // "not scored" state below rather than the Match row riding alongside
  // Coverage under a contradictory badge.
  const hasSemantic =
    isMeasured(score) && score.scoreSource === 'combined' && Number.isFinite(score.semantic);
  // The kernel's own detail sentence — the one place the job's keyword COUNT
  // (the denominator "coverage" is a fraction of) is surfaced; it always ends
  // with the same disclaimer already shown, translated, in `jobs.scoreGuidance`
  // above, so that exact echoed tail (given back to us as `score.guidance`,
  // not hardcoded English here) is trimmed to avoid saying it twice.
  const explanationText =
    isMeasured(score) && score.explanation
      ? score.guidance && score.explanation.endsWith(score.guidance)
        ? score.explanation.slice(0, -score.guidance.length).trim()
        : score.explanation
      : undefined;

  if (!active) return null;

  return (
    <div
      data-testid={TEST_IDS.documents.jobAdViewScorePanel}
      className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-3"
    >
      <p className="shrink-0 text-[10px] leading-relaxed text-foreground/70">
        {t('jobs.scoreGuidance')}
      </p>
      {/* This surface can only ever score the ORIGINAL saved résumé (see
          `resumeId`'s doc) — but the wizard/results panel one view over
          just generated a TAILORED document, so without this the number
          here reads as scoring that instead. */}
      {!!resumeId && (
        <p className="shrink-0 text-[10px] leading-relaxed text-foreground/70">
          {t('autopilot.apply.jobAdView.score.resumeNote')}
        </p>
      )}
      {!resumeId ? (
        <EmptyState icon={FileText} title={t('jobs.scoreNoResume')} className="flex-1 py-6" />
      ) : !scoreText ? (
        <EmptyState
          icon={FileSearch}
          title={t('autopilot.apply.jobAdView.score.noPosting')}
          className="flex-1 py-6"
        />
      ) : scoreLoading ? (
        // A translating scoring call can take up to ~117s (see
        // `handleTabChange`'s doc) — the one state on this tab most in
        // need of a live-region announcement, unlike the Summary tab's
        // `generating` block which it mirrors.
        <>
          {egressNotice}
          <div
            role="status"
            aria-live="polite"
            className="flex items-center gap-2 text-[11px] text-foreground/40"
          >
            <Loader2 size={12} className="animate-spin" />
            {t('autopilot.apply.jobAdView.score.loading')}
          </div>
        </>
      ) : scoreError || (score && !isMeasured(score)) ? (
        // Two distinct failure shapes routed to the SAME honest state: a
        // rejected query (offline, provider outage, the 200_000-byte zod
        // cap) and a *resolved* one that isn't actually a MatchScore (see
        // `isMeasured`'s doc). Neither is "not scored" — that copy would
        // tell a user who waited up to two minutes for a failure exactly
        // what they'd be told about a posting with nothing in it.
        <>
          {egressNotice}
          <ErrorState
            title={t('autopilot.apply.jobAdView.score.errorTitle')}
            description={t('autopilot.apply.jobAdView.score.errorDescription')}
            onRetry={() => {
              void refetchScore();
            }}
            className="rounded-lg border border-red-400/20 bg-red-400/5 py-6"
          />
        </>
      ) : score ? (
        <>
          {egressNotice}
          <div className="flex flex-col gap-1.5">
            {/* `semantic_enabled` is hardcoded off for this endpoint (see
              `hasSemantic`'s doc), so `combined === ats` always — Match and
              Coverage would print the identical number under DIFFERENT
              badge cut points (combined 75/50 vs coverage 55/30),
              contradicting each other on every render. Drop Match; it
              carries no information Coverage doesn't, and "Keyword
              coverage" is the honest label for a keyword-only number. */}
            {hasSemantic && (
              <ScoreMetric
                label={t('autopilot.scoreAbbr.combined')}
                value={hasCoverage ? score.combined : null}
                variant="combined"
                notScoredLabel={t('autopilot.apply.jobAdView.score.noKeywords')}
                testId={TEST_IDS.documents.jobAdViewScoreMatch}
              />
            )}
            <ScoreMetric
              // Reuses the Autopilot list's own field labels (`autopilot.scoreAbbr.*`)
              // rather than forking a second translation for the identical
              // `MatchScore.combined`/`.ats` values — the naming rule this
              // surface already follows for "never ATS score", applied to
              // German too.
              label={t('autopilot.scoreAbbr.coverage')}
              value={hasCoverage ? score.ats : null}
              variant="coverage"
              notScoredLabel={t('autopilot.apply.jobAdView.score.noKeywords')}
              testId={TEST_IDS.documents.jobAdViewScoreCoverage}
            />
            {hasSemantic ? (
              <ScoreMetric
                label={t('autopilot.apply.jobAdView.score.semanticLabel')}
                value={score.semantic}
                notScoredLabel={t('analyze.notScored')}
                testId={TEST_IDS.documents.jobAdViewScoreSemantic}
              />
            ) : (
              // Reached whenever semantic scoring is off by preference OR
              // was requested but degraded (see `hasSemantic`'s doc) — a
              // disclosure footnote, not a metric row: reserving a full row
              // with badge chrome for a value that has no content would be
              // its own small dishonesty. Either way `explanationText`
              // below (echoing the kernel's own `explanation` sentence)
              // states the reason — "semantic scoring disabled" or
              // "semantic similarity could not be computed — no embedding
              // was available for this pair" — so the degrade reason is
              // never silently dropped, just not duplicated into this
              // footnote's own label.
              <p
                data-testid={TEST_IDS.documents.jobAdViewScoreSemantic}
                className="text-[10px] text-foreground/50"
              >
                {t('autopilot.apply.jobAdView.score.semanticLabel')}: {t('analyze.notScored')}
              </p>
            )}
            {/* The kernel's own detail sentence — the keyword COUNT the
              coverage fraction is out of, the one signal that lets a user
              notice a boilerplate-inflated denominator. Backend-authored
              prose (like `error` above), deliberately not run through
              `t()` — there is nothing to translate a runtime-interpolated
              English sentence INTO. */}
            {explanationText && (
              <p className="text-[10px] leading-relaxed text-foreground/50">{explanationText}</p>
            )}
            {hasCoverage && score.gaps.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="text-[10px] text-foreground/50">{t('analyze.gaps')}</span>
                <div className="flex flex-wrap gap-1">
                  {score.gaps.slice(0, 3).map((gap) => (
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
            {/* Backend-authored English prose (like `explanationText`
              above), deliberately not run through `t()`. Gated on
              `hasCoverage` — the placeholder "no extractable keywords"
              result (`ats: 0, gaps: []`) still produces a "Strong
              keyword coverage" recommendation from the same `gaps`
              input, which would be a false positive here. */}
            {hasCoverage && score.recommendations.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="text-[10px] text-foreground/50">
                  {t('analyze.recommendations')}
                </span>
                {score.recommendations.map((rec) => (
                  <p key={rec} className="text-[10px] leading-relaxed text-foreground/50">
                    {rec}
                  </p>
                ))}
              </div>
            )}
          </div>
        </>
      ) : (
        <p className="text-[11px] text-foreground/50">{t('analyze.notScored')}</p>
      )}
    </div>
  );
}
