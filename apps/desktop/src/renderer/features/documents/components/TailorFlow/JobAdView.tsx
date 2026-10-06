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

import { JobAdScorePanel } from './JobAdView/JobAdScorePanel';

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
        ) : tab === 'score' ? null : fetchingDesc ? (
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
        {/* Always mounted (renders null off-tab) so the score query observer keeps its
            pre-split lifetime: mounted-but-disabled off-tab, cache never starts gc. */}
        <JobAdScorePanel
          active={tab === 'score'}
          resumeId={resumeId}
          scoreSnapshot={scoreSnapshot}
        />
      </div>
    </div>
  );
}
