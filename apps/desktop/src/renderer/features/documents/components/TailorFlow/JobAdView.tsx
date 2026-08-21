import {
  ExternalLink as ExternalLinkIcon,
  FileSearch,
  FileText,
  Loader2,
  Sparkles,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  Alert,
  Button,
  Dropdown,
  EmptyState,
  ErrorState,
  MarkdownMessage,
  SegmentedControl,
  StreamingText,
  TextArea,
} from '@ajh/ui';

import { ExternalLink } from '@/components/ui/ExternalLink';
import { ModelSelector, useSelectedProvider } from '@/components/ui/ModelSelector';
import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { OUTPUT_LANGUAGES } from '@/lib/generate';
import { useJobAdTextMatchScore } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

import { hasScoreCoverage, isMeasured, ScoreMetric } from './MatchScoreMetric';

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
}

/** Returns true when the text ends with the ellipsis character or three dots. */
function looksPartial(text: string) {
  const t = text.trimEnd();
  return t.endsWith('…') || t.endsWith('...');
}

/**
 * Shared job-ad surface (Summary | Job Ad | Score) used by both the wizard's
 * first step and the results panel's job-ad tab. The Summary sub-tab lazily
 * streams an AI summary on an explicit click; the Job Ad sub-tab shows the raw
 * posting as an EDITABLE textarea so a bad scrape can be fixed before tailoring;
 * the Score sub-tab scores the stored résumé against a SNAPSHOT of this posting's
 * text, taken the instant the tab is opened (never the live, still-editable
 * text — see `handleTabChange`) — the "before" half of a comparison in the
 * results panel, a plain readout in the wizard.
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
}: Props) {
  const { t } = useTranslation();

  // Score-tab egress disclosure: scoring a foreign-language posting routes
  // through translation, and the CLI-agent providers (Claude Code, Codex,
  // Gemini CLI) egress despite reading as "local" elsewhere in this app —
  // see the Score tab's guidance paragraph below.
  const activeProvider = useSelectedProvider();
  const activeProviderMeta = PROVIDERS[activeProvider as AiProvider];
  const isCliAgentProvider = activeProviderMeta?.kind === 'cli-agent';

  // Start on `source` when there's nothing to show or the snippet is truncated —
  // that's when paste is the most useful action. `summary` otherwise (normal case).
  // Never defaults to `score` — that tab is opt-in via an explicit click.
  const truncated = looksPartial(jobDesc);
  const [tab, setTab] = useState<'summary' | 'source' | 'score'>(
    !hasDesc || truncated ? 'source' : 'summary'
  );

  // A SNAPSHOT of the posting text, taken the instant the Score tab is
  // opened — never `jobDesc` live. `useJobAdTextMatchScore`'s query key is
  // content-addressed on the text itself, and the Job Ad sub-tab right next
  // to this one is an editable textarea; wiring the query straight to
  // `jobDesc` would mint (and fire) a fresh query key on every keystroke.
  // Worse, this surface TRANSLATES (`MatchSurface::JobAdText`), so a
  // foreign-language posting can reach a local model — a slow one measured at
  // 117s for a single call. Snapshotting on open (an event, not an effect —
  // there's no external system to sync with) means typing never re-scores;
  // re-opening the tab does.
  const [scoreSnapshot, setScoreSnapshot] = useState<string | null>(null);
  const handleTabChange = (next: 'summary' | 'source' | 'score') => {
    if (next === 'score') setScoreSnapshot(jobDesc);
    setTab(next);
  };

  // Lazy: only fires once the Score tab is open, a résumé is stored, AND the
  // snapshot has real text — a whitespace-only paste is still "empty" (the
  // IPC schema only requires length >= 1, so this guard is the real check).
  const scoreText = scoreSnapshot?.trim() ?? '';
  const scoreEnabled = tab === 'score' && !!resumeId && !!scoreText;
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

  // Re-pick the default sub-tab only when the POSTING changes (new jobUrl), not on
  // every jobDesc edit — pasting into the source textarea changes `truncated`, and
  // resyncing on that would yank the user out of the textarea they're editing.
  const prevJobUrl = useRef(jobUrl);
  useEffect(() => {
    if (prevJobUrl.current === jobUrl) return;
    prevJobUrl.current = jobUrl;
    // Two postings can share a jobUrl (or both have none — manually-added
    // applications) without sharing TEXT; an unreset snapshot would render
    // posting A's score against posting B's textarea.
    setScoreSnapshot(null);
    setTab(!hasDesc || looksPartial(jobDesc) ? 'source' : 'summary');
  }, [jobUrl, hasDesc, jobDesc]);

  // Sourced from OUTPUT_LANGUAGES (the single locale source of truth) so each value
  // is a locale CODE the generation pipeline's safeLocale accepts — display names
  // ('German', 'Dutch') silently collapsed to English. Labels are endonyms, each
  // language shown in its own script.
  const languageOptions = OUTPUT_LANGUAGES.map((l) => ({ value: l.code, label: l.endonym }));

  // A CLI-agent provider (Claude Code, Codex, Gemini CLI) egresses despite
  // reading as "local" elsewhere in this app — scoring a foreign-language
  // posting routes through translation, which sends the job ad text to it.
  // Rendered on every branch that can only be reached AFTER a request went out
  // — loading, measured, and error. The error branch counts: its second
  // disjunct (`score && !isMeasured(score)`) requires the query to have
  // RESOLVED, so the round trip provably completed and the posting text was
  // already sent. Only the no-résumé/no-posting branches are silent, because
  // those short-circuit before `scoreEnabled` ever turns on. Computed once here
  // so all three call sites stay in sync.
  const egressNotice = isCliAgentProvider ? (
    <p className="shrink-0 text-[10px] leading-relaxed text-foreground/70">
      {t('autopilot.apply.jobAdView.score.cliAgentEgress', {
        provider: activeProviderMeta?.label ?? activeProvider,
      })}
    </p>
  ) : null;

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
          onChange={handleTabChange}
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
          // a plain readout in the wizard. Every number here traces back to
          // ONE `MatchScore`, for the snapshot taken when this tab opened; an
          // absent input is a stated reason, never a `0`. Estimate framing
          // matches the Jobs page (`jobs.scoreGuidance`).
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
              // `generating` block five lines up which it mirrors.
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
                    <p className="text-[10px] leading-relaxed text-foreground/50">
                      {explanationText}
                    </p>
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
