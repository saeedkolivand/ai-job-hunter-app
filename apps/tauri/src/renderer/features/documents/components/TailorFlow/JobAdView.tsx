import { ExternalLink as ExternalLinkIcon, Loader2, Sparkles } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import {
  Button,
  Dropdown,
  MarkdownMessage,
  SegmentedControl,
  StreamingText,
  TextArea,
} from '@ajh/ui';

import { ExternalLink } from '@/components/ui/ExternalLink';
import { OUTPUT_LANGUAGES } from '@/lib/generate';

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
}

/**
 * Shared two-tab job-ad surface (Summary | Job Ad) used by both the wizard's
 * first step and the results panel's job-ad tab. The Summary sub-tab lazily
 * streams an AI summary on an explicit click; the Job Ad sub-tab shows the raw
 * posting as an EDITABLE textarea so a bad scrape can be fixed before tailoring.
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
}: Props) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<'summary' | 'source'>('summary');

  // Sourced from OUTPUT_LANGUAGES (the single locale source of truth) so each value
  // is a locale CODE the generation pipeline's safeLocale accepts — display names
  // ('German', 'Dutch') silently collapsed to English. Labels are endonyms, each
  // language shown in its own script.
  const languageOptions = OUTPUT_LANGUAGES.map((l) => ({ value: l.code, label: l.endonym }));

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="shrink-0 flex items-center justify-between gap-2">
        <SegmentedControl<'summary' | 'source'>
          options={[
            { value: 'summary', label: t('autopilot.apply.jobAdView.summaryTab') },
            { value: 'source', label: t('autopilot.apply.tabs.jobAd') },
          ]}
          value={tab}
          onChange={setTab}
          size="sm"
          ariaLabel={t('autopilot.apply.jobAdView.label')}
        />
        {tab === 'summary' && (
          <>
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
          </>
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
                className="min-h-0 flex-1 select-text overflow-y-auto rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2"
              >
                <StreamingText
                  text={summary}
                  isStreaming
                  className="text-[11px] leading-relaxed text-foreground/70"
                />
              </div>
            ) : summary ? (
              <div className="min-h-0 flex-1 select-text overflow-y-auto rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/70">
                <MarkdownMessage content={summary} />
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-white/[0.06] bg-white/[0.02] px-6 py-8 text-center">
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
        ) : fetchingDesc ? (
          <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] text-foreground/40">
            <Loader2 size={12} className="animate-spin" />
            {t('autopilot.apply.fetchingDescription')}
          </div>
        ) : hasDesc || jobDesc ? (
          <div className="flex min-h-0 flex-1 flex-col gap-1">
            <TextArea
              variant="glass"
              value={jobDesc}
              onChange={(e) => onJobDescChange(e.target.value)}
              className="h-full flex-1 resize-none text-[11px] leading-relaxed"
              aria-label={t('autopilot.apply.jobAdView.editHelper')}
            />
            <p className="shrink-0 text-[10px] text-foreground/35">
              {t('autopilot.apply.jobAdView.editHelper')}
            </p>
          </div>
        ) : jobUrl ? (
          <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
            {t('autopilot.apply.loadFailed')}{' '}
            <ExternalLink
              href={jobUrl}
              className="inline-flex items-center gap-0.5 font-medium text-brand-soft hover:underline"
            >
              {t('autopilot.viewJob')}
              <ExternalLinkIcon size={10} />
            </ExternalLink>
          </div>
        ) : (
          <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
            {t('autopilot.apply.noDescription')}
          </div>
        )}
      </div>
    </div>
  );
}
