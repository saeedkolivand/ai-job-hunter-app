import { useTranslation } from '@ajh/translations';

import { JobDescriptionPanel } from '../../JobDescriptionPanel';

interface StepJobAdProps {
  jobDesc: string;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
}

/**
 * Read-only first step: the job posting being tailored against. No form field —
 * "Next" is always enabled. Reuses {@link JobDescriptionPanel} in `fill` mode for
 * the fetching / has-desc / load-failed / no-desc states.
 */
export function StepJobAd({ jobDesc, hasDesc, fetchingDesc, jobUrl }: StepJobAdProps) {
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
      <JobDescriptionPanel
        jobDesc={jobDesc}
        hasDesc={hasDesc}
        fetchingDesc={fetchingDesc}
        jobUrl={jobUrl}
        fill
      />
    </div>
  );
}
