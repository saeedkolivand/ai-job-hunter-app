import { Briefcase } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Input, SettingsSection } from '@ajh/ui';

import type { ApplicantPreferences } from '@/store/preferences-schema';
import { useApplicant, usePreferencesStore } from '@/store/preferences-store';

const FIELDS: {
  key: keyof ApplicantPreferences;
  labelKey: string;
  placeholderKey: string;
}[] = [
  {
    key: 'salaryExpectation',
    labelKey: 'settings.applicant.salary',
    placeholderKey: 'settings.applicant.salaryPlaceholder',
  },
  {
    key: 'earliestStartDate',
    labelKey: 'settings.applicant.startDate',
    placeholderKey: 'settings.applicant.startDatePlaceholder',
  },
  {
    key: 'noticePeriod',
    labelKey: 'settings.applicant.notice',
    placeholderKey: 'settings.applicant.noticePlaceholder',
  },
  {
    key: 'remotePreference',
    labelKey: 'settings.applicant.remote',
    placeholderKey: 'settings.applicant.remotePlaceholder',
  },
];

/**
 * Applicant details — user-supplied facts a résumé can't answer (salary, start
 * date, notice, remote). Fed into the cover letter (market inclusions such as
 * the DACH salary expectation + earliest start date) and autopilot application
 * answers. User-supplied ONLY: the generators state these where the market or
 * question calls for them, and never invent them when blank.
 */
export function ApplicantDetailsSection() {
  const { t } = useTranslation();
  const applicant = useApplicant();
  const setApplicant = usePreferencesStore((s) => s.setApplicant);

  const update = (key: keyof ApplicantPreferences, value: string) => {
    const next: ApplicantPreferences = { ...applicant, [key]: value };
    setApplicant(next);
  };

  return (
    <SettingsSection icon={Briefcase} label={t('settings.applicant.title')}>
      <p className="mb-4 text-xs text-foreground/55">{t('settings.applicant.description')}</p>
      <div className="space-y-3">
        {FIELDS.map(({ key, labelKey, placeholderKey }) => (
          <label key={key} className="block">
            <span className="mb-1 block text-xs font-medium text-foreground/70">{t(labelKey)}</span>
            <Input
              className="w-full"
              value={applicant?.[key] ?? ''}
              onChange={(e) => update(key, e.target.value)}
              placeholder={t(placeholderKey)}
            />
          </label>
        ))}
      </div>
    </SettingsSection>
  );
}
