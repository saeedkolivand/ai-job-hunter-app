import type { Autopilot, JobPreferences } from '@ajh/shared';

import type { WizardState } from '@/features/autopilot/types';

/** Initial wizard form, pre-filled from saved job preferences where available. */
export function buildDefaults(jobPrefs?: JobPreferences): WizardState {
  const validWorkType = ['remote', 'hybrid', 'on-site', 'any'] as const;
  return {
    name: '',
    board: 'linkedin',
    query: '',
    location: jobPrefs?.location ?? '',
    workType: validWorkType.includes(jobPrefs?.remote as (typeof validWorkType)[number])
      ? (jobPrefs?.remote as WizardState['workType'])
      : 'any',
    pages: 2,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: jobPrefs?.techStack?.map((t) => t.name).join(', ') ?? '',
    excludeKeywords: '',
    resumeText: '',
    action: 'save',
    coverLetter: '',
    autoSubmit: false,
    schedule: 'daily',
  };
}

/** Map a persisted autopilot back into the wizard form for editing. */
export function autopilotToWizardState(ap: Autopilot): WizardState {
  return {
    name: ap.name,
    board: ap.target.board,
    query: ap.target.query,
    location: ap.target.location ?? '',
    workType: ap.target.workType ?? 'any',
    pages: ap.target.pages,
    dateFilter: ap.target.dateFilter ?? '',
    minMatchScore: ap.filter.minMatchScore,
    keywords: ap.filter.keywords?.join(', ') ?? '',
    excludeKeywords: ap.filter.excludeKeywords?.join(', ') ?? '',
    resumeText: ap.resumeText ?? '',
    action: ap.action,
    coverLetter: ap.coverLetter ?? '',
    autoSubmit: ap.autoSubmit,
    schedule: ap.schedule,
  };
}
