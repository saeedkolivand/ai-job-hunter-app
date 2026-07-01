import { useTranslation } from '@ajh/translations';

import { JobAdView } from '../../JobAdView';

interface StepJobAdProps {
  jobDesc: string;
  onJobDescChange: (v: string) => void;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
  jobAdSummary: {
    summary: string;
    generating: boolean;
    error: string | null;
    generate: () => void;
    language: string;
    setLanguage: (v: string) => void;
  };
}

/**
 * First step: an editable two-tab job-ad view (Summary | Job Ad) via
 * {@link JobAdView}. No form field — "Next" is always enabled. The Summary tab
 * lazily streams an AI summary; the Job Ad tab lets the user fix a bad scrape
 * before tailoring (edits flow up via `onJobDescChange`).
 */
export function StepJobAd({
  jobDesc,
  onJobDescChange,
  hasDesc,
  fetchingDesc,
  jobUrl,
  jobAdSummary,
}: StepJobAdProps) {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="shrink-0">
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.apply.wizard.jobAd.title')}
        </p>
        <p className="mt-0.5 text-xs text-foreground/35">
          {t('autopilot.apply.wizard.jobAd.subtitle')}
        </p>
      </div>
      <JobAdView
        jobDesc={jobDesc}
        onJobDescChange={onJobDescChange}
        summary={jobAdSummary.summary}
        generating={jobAdSummary.generating}
        error={jobAdSummary.error}
        onGenerateSummary={jobAdSummary.generate}
        language={jobAdSummary.language}
        onLanguageChange={jobAdSummary.setLanguage}
        hasDesc={hasDesc}
        fetchingDesc={fetchingDesc}
        jobUrl={jobUrl}
      />
    </div>
  );
}
