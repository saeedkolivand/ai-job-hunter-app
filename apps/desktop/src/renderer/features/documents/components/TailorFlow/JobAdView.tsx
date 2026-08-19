import { ExternalLink as ExternalLinkIcon, Loader2, Sparkles } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  Alert,
  Button,
  Dropdown,
  MarkdownMessage,
  SegmentedControl,
  StreamingText,
  TextArea,
} from '@ajh/ui';

import { ExternalLink } from '@/components/ui/ExternalLink';
import { ModelSelector } from '@/components/ui/ModelSelector';
import { OUTPUT_LANGUAGES } from '@/lib/generate';
import { MatchBand } from '@/lib/match-band';
import { useJobMatchScore } from '@/services';

interface Props {
  jobDesc: string;
  onJobDescChange: (v: string) => void;
  summary: string;
  generating: boolean;
  error: string | null;
  onGenerateSummary: () => void;
  language: string;
  onLanguageChange: (v: string) => void;
  hasDesc: boolean;
  fetchingDesc?: boolean;
  jobUrl?: string;
  /** Saved résumé backing this generation (the ORIGINAL, unedited résumé — not
   *  the wizard's live/tailored text). Undefined where no saved résumé is
   *  threaded yet — the Score tab then shows a stated reason, never a `0`. */
  resumeId?: string;
  /** Local postings-cache id for this job — the SAME id `useJobMatchScore`
   *  reads on the Jobs page, so this tab can never compute its own, drifting
   *  number. Undefined where no such id is threaded yet (see the Score tab's
   *  empty state). */
  jobId?: string;
}

/** Returns true when the text ends with the ellipsis character or three dots. */
function looksPartial(text: string) {
  const t = text.trimEnd();
  return t.endsWith('…') || t.endsWith('...');
}

interface ScoreMetricProps {
  label: string;
  /** `null` for "not actually measured" — renders `notScoredLabel` instead of
   *  a fabricated `0`. Never pass a real `0` measurement as `null`. */
  value: number | null;
  variant?: 'combined' | 'coverage';
  notScoredLabel: string;
  testId: string;
}

/** One Score-tab row: a real percentage + tier badge, or an honest
 *  "not scored" placeholder — see the tab's callers for what makes a value
 *  real vs. a placeholder. */
function ScoreMetric({ label, value, variant, notScoredLabel, testId }: ScoreMetricProps) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2">
      <span className="text-[11px] text-foreground/60">{label}</span>
      <span data-testid={testId} className="flex items-center gap-1.5">
        {value === null ? (
          <span className="text-[11px] font-medium text-foreground/40">{notScoredLabel}</span>
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

/**
 * Shared job-ad surface (Summary | Job Ad | Score) used by both the wizard's
 * first step and the results panel's job-ad tab. The Summary sub-tab lazily
 * streams an AI summary on an explicit click; the Job Ad sub-tab shows the raw
 * posting as an EDITABLE textarea so a bad scrape can be fixed before tailoring;
 * the Score sub-tab reads the stored résumé's cached `MatchScore` against this
 * posting — the "before" half of a comparison in the results panel, a plain
 * readout in the wizard.
 *
 * Default tab is `source` when the description is missing or looks truncated so
 * paste is immediately discoverable; otherwise `summary`. `score` is never the
 * default — it's opt-in.
 */
export function JobAdView({
  jobDesc,
  onJobDescChange,
  summary,
  generating,
  error,
  onGenerateSummary,
  language,
  onLanguageChange,
  hasDesc,
  fetchingDesc,
  jobUrl,
  resumeId,
  jobId,
}: Props) {
  const { t } = useTranslation();

  // Start on `source` when there's nothing to show or the snippet is truncated —
  // that's when paste is the most useful action. `summary` otherwise (normal case).
  // Never defaults to `score` — that tab is opt-in via an explicit click.
  const truncated = looksPartial(jobDesc);
  const [tab, setTab] = useState<'summary' | 'source' | 'score'>(
    !hasDesc || truncated ? 'source' : 'summary'
  );

  // Same cached query the Jobs page reads (`['match', resumeId, jobId, …]`,
  // 10-min staleTime) — this tab can only ever show that number, never a
  // second, possibly-drifted computation. Lazy: only fires once the Score tab
  // is actually opened, and only when both ids are known.
  const scoreEnabled = tab === 'score' && !!resumeId && !!jobId;
  const { data: score, isLoading: scoreLoading } = useJobMatchScore(
    resumeId ?? null,
    jobId ?? '',
    scoreEnabled
  );
  // `keyword_coverage` (match_resume.rs) returns `ats: 0, gaps: []` for BOTH
  // "no extractable keywords" AND would for a genuine 0% match — except a
  // genuine 0% still lists every job keyword as a gap, so `gaps.length === 0`
  // only co-occurs with `ats === 0` in the former case. `combined` inherits
  // the same placeholder (its formula always needs a real `ats` input), so
  // this gates the Match row too — never a fake `0`.
  const hasCoverage = !!score && !(score.ats === 0 && score.gaps.length === 0);
  // Semantic scoring is opt-in and off by default — `semantic` is a real
  // number either way, but only a genuine measurement when the backend says
  // the combined kernel actually ran (see `MatchScore.scoreSource`'s doc).
  const hasSemantic = score?.scoreSource === 'combined';

  // Re-pick the default sub-tab only when the POSTING changes (new jobUrl), not on
  // every jobDesc edit — pasting into the source textarea changes `truncated`, and
  // resyncing on that would yank the user out of the textarea they're editing.
  const prevJobUrl = useRef(jobUrl);
  useEffect(() => {
    if (prevJobUrl.current === jobUrl) return;
    prevJobUrl.current = jobUrl;
    setTab(!hasDesc || looksPartial(jobDesc) ? 'source' : 'summary');
  }, [jobUrl, hasDesc, jobDesc]);

  // Sourced from OUTPUT_LANGUAGES (the single locale source of truth) so each value
  // is a locale CODE the generation pipeline's safeLocale accepts — display names
  // ('German', 'Dutch') silently collapsed to English. Labels are endonyms, each
  // language shown in its own script.
  const languageOptions = OUTPUT_LANGUAGES.map((l) => ({ value: l.code, label: l.endonym }));

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="shrink-0 flex items-center justify-between gap-2">
        <SegmentedControl<'summary' | 'source' | 'score'>
          options={[
            { value: 'summary', label: t('autopilot.apply.jobAdView.summaryTab') },
            { value: 'source', label: t('autopilot.apply.tabs.jobAd') },
            { value: 'score', label: t('autopilot.apply.jobAdView.scoreTab') },
          ]}
          value={tab}
          onChange={setTab}
          size="sm"
          ariaLabel={t('autopilot.apply.jobAdView.label')}
        />
        {tab === 'summary' && (
          <div className="flex min-w-0 items-center gap-2">
            {/* Explicit label bound to the trigger (id) — visually redundant with
                the selected language, so sr-only keeps the toolbar uncluttered. */}
            <label htmlFor="job-ad-summary-language" className="sr-only">
              {t('autopilot.apply.jobAdView.summaryLanguage')}
            </label>
            <Dropdown
              id="job-ad-summary-language"
              value={language}
              onChange={onLanguageChange}
              options={languageOptions}
              size="sm"
            />
            {/* `min-w-0` lets it shrink below its content (model label + guidance
                line) instead of pushing past the card edge; no `flex-1` — that would
                also stretch it to fill the row on wide cards, beyond this bug fix. */}
            <ModelSelector className="min-w-0" />
          </div>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col">
        {tab === 'summary' ? (
          <div className="flex min-h-0 flex-1 flex-col gap-2">
            {error && (
              <div className="shrink-0 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-[11px] text-red-300/80">
                {error}
              </div>
            )}
            {generating ? (
              <div
                role="status"
                aria-live="polite"
                aria-label={t('autopilot.apply.jobAdView.generating')}
                className="min-h-0 flex-1 select-text overflow-y-auto rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2"
              >
                <StreamingText
                  text={summary}
                  isStreaming
                  className="text-[11px] leading-relaxed text-foreground/70"
                />
              </div>
            ) : summary ? (
              <div className="min-h-0 flex-1 select-text overflow-y-auto rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/70">
                <MarkdownMessage content={summary} />
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-6 py-8 text-center">
                <p className="text-[11px] leading-relaxed text-foreground/40">
                  {t('autopilot.apply.jobAdView.summaryHint')}
                </p>
                <Button
                  variant="primary"
                  onClick={onGenerateSummary}
                  disabled={!hasDesc}
                  className="gap-1.5"
                >
                  <Sparkles size={13} /> {t('autopilot.apply.jobAdView.generateSummary')}
                </Button>
              </div>
            )}
          </div>
        ) : tab === 'score' ? (
          // Score tab — the "before" half of a comparison in the results panel,
          // a plain readout in the wizard. Every number here traces back to the
          // one cached `MatchScore`; an absent input is a stated reason, never
          // a `0`. Estimate framing matches the Jobs page (`jobs.scoreGuidance`).
          <div
            data-testid={TEST_IDS.documents.jobAdViewScorePanel}
            className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-3"
          >
            <p className="shrink-0 text-[10px] leading-relaxed text-foreground/40">
              {t('jobs.scoreGuidance')}
            </p>
            {!resumeId ? (
              <p className="text-[11px] text-foreground/50">{t('jobs.scoreNoResume')}</p>
            ) : !jobId ? (
              <p className="text-[11px] text-foreground/50">
                {t('autopilot.apply.jobAdView.score.noJob')}
              </p>
            ) : scoreLoading ? (
              <div className="flex items-center gap-2 text-[11px] text-foreground/40">
                <Loader2 size={12} className="animate-spin" />
                {t('autopilot.loading')}
              </div>
            ) : score ? (
              <div className="flex flex-col gap-1.5">
                <ScoreMetric
                  label={t('autopilot.apply.jobAdView.score.matchLabel')}
                  value={hasCoverage ? score.combined : null}
                  variant="combined"
                  notScoredLabel={t('autopilot.apply.jobAdView.score.noKeywords')}
                  testId={TEST_IDS.documents.jobAdViewScoreMatch}
                />
                <ScoreMetric
                  label={t('autopilot.apply.jobAdView.score.coverageLabel')}
                  value={hasCoverage ? score.ats : null}
                  variant="coverage"
                  notScoredLabel={t('autopilot.apply.jobAdView.score.noKeywords')}
                  testId={TEST_IDS.documents.jobAdViewScoreCoverage}
                />
                <ScoreMetric
                  label={t('autopilot.apply.jobAdView.score.semanticLabel')}
                  value={hasSemantic ? score.semantic : null}
                  notScoredLabel={t('analyze.notScored')}
                  testId={TEST_IDS.documents.jobAdViewScoreSemantic}
                />
              </div>
            ) : (
              <p className="text-[11px] text-foreground/50">{t('analyze.notScored')}</p>
            )}
          </div>
        ) : fetchingDesc ? (
          <div className="flex items-center gap-2 rounded-lg border border-foreground/[0.06] bg-foreground/[0.02] px-3 py-2 text-[11px] text-foreground/40">
            <Loader2 size={12} className="animate-spin" />
            {t('autopilot.apply.fetchingDescription')}
          </div>
        ) : (
          // Source tab — always editable. Empty when scrape failed / no description captured.
          <div className="flex min-h-0 flex-1 flex-col gap-1">
            {truncated && (
              <Alert
                type="warning"
                message={t('autopilot.apply.jobAdView.truncatedHint')}
                className="shrink-0"
              />
            )}
            <TextArea
              variant="glass"
              value={jobDesc}
              onChange={(e) => onJobDescChange(e.target.value)}
              placeholder={t('autopilot.apply.jobAdView.pasteHint')}
              className="h-full flex-1 resize-none text-[11px] leading-relaxed shadow-none"
              aria-label={t('autopilot.apply.tabs.jobAd')}
              aria-describedby="job-ad-edit-helper"
              data-testid={TEST_IDS.documents.jobAdViewTextarea}
            />
            <p id="job-ad-edit-helper" className="shrink-0 text-[10px] text-foreground/35">
              {t('autopilot.apply.jobAdView.editHelper')}
            </p>
            {jobUrl && (
              <ExternalLink
                href={jobUrl}
                className="shrink-0 inline-flex items-center gap-0.5 self-start text-[10px] font-medium text-brand-soft hover:underline"
              >
                {t('autopilot.viewJob')}
                <ExternalLinkIcon size={10} />
              </ExternalLink>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
