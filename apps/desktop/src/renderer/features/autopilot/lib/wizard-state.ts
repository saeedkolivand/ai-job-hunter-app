import {
  AGGREGATOR_BOARD_ID,
  type Autopilot,
  type AutopilotCreate,
  type JobPreferences,
} from '@ajh/shared';

import type { WizardState } from '@/features/autopilot/types';

/** Split a comma-separated keyword string into a trimmed, non-empty list (or undefined). */
function splitKeywords(raw: string): string[] | undefined {
  const list = raw
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  return list.length > 0 ? list : undefined;
}

/**
 * Map the wizard form into the IPC create/update payload. Pure — the single place
 * the form's editing-layer sentinels collapse into the wire shape:
 * comma keywords → arrays, `workType: 'any'` → undefined, an empty `dateFilter`/
 * `location`/`resumeText` → undefined, and a `manual` schedule drops its time.
 */
export function wizardStateToPayload(form: WizardState): AutopilotCreate {
  return {
    name: form.name,
    target: {
      boards: form.boards,
      query: form.query,
      location: form.location || undefined,
      countryCode: form.countryCode || undefined,
      // Empty array = the form's "any" sentinel; collapse to undefined so an
      // old autopilot round-trips clean, matching the `dateFilter` precedent below.
      workTypes: form.workTypes.length > 0 ? form.workTypes : undefined,
      pages: form.pages,
      dateFilter: form.dateFilter || undefined,
      // Off collapses to undefined so an old autopilot stays free of the field.
      watchedCompaniesOnly: form.watchedCompaniesOnly || undefined,
    },
    filter: {
      minMatchScore: form.minMatchScore,
      keywords: splitKeywords(form.keywords),
      excludeKeywords: splitKeywords(form.excludeKeywords),
    },
    resumeText: form.resumeText || undefined,
    assistant: form.assistant,
    // Only forward the snapshot while notes are on — an off toggle carries no
    // meaning for a stale provider/model/baseUrl.
    assistantProvider: form.assistant ? form.assistantProvider : undefined,
    assistantModel: form.assistant ? form.assistantModel : undefined,
    assistantBaseUrl: form.assistant ? form.assistantBaseUrl : undefined,
    schedule: form.schedule,
    // Time-of-day only applies to recurring schedules; manual runs have no time.
    scheduleHour: form.schedule === 'manual' ? undefined : form.scheduleHour,
    scheduleMinute: form.schedule === 'manual' ? undefined : form.scheduleMinute,
  };
}

/** Initial wizard form, pre-filled from saved job preferences where available. */
export function buildDefaults(jobPrefs?: JobPreferences): WizardState {
  return {
    name: '',
    boards: [AGGREGATOR_BOARD_ID],
    query: '',
    location: jobPrefs?.location ?? '',
    // Seeded alongside `location` (autopilot aggregator zero-jobs fix) so a
    // saved preferred location keeps its real country instead of the
    // aggregator having to guess one at scrape time.
    countryCode: jobPrefs?.countryCode,
    // No job-preference field seeds work type; default to the empty "any" set.
    workTypes: [],
    // Matches the backend's AutopilotTargetSchema.pages default.
    pages: 2,
    dateFilter: '',
    watchedCompaniesOnly: false,
    minMatchScore: 0,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    assistant: false,
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
  };
}

/** Map a persisted autopilot back into the wizard form for editing. */
export function autopilotToWizardState(ap: Autopilot): WizardState {
  const { target } = ap;
  const boards: string[] = target.boards.length > 0 ? target.boards : [AGGREGATOR_BOARD_ID];
  return {
    name: ap.name,
    boards,
    query: target.query,
    location: target.location ?? '',
    countryCode: target.countryCode,
    workTypes: target.workTypes ?? [],
    pages: target.pages,
    dateFilter: target.dateFilter ?? '',
    watchedCompaniesOnly: target.watchedCompaniesOnly ?? false,
    minMatchScore: ap.filter.minMatchScore,
    keywords: ap.filter.keywords?.join(', ') ?? '',
    excludeKeywords: ap.filter.excludeKeywords?.join(', ') ?? '',
    resumeText: ap.resumeText ?? '',
    assistant: ap.assistant ?? false,
    assistantProvider: ap.assistantProvider,
    assistantModel: ap.assistantModel,
    assistantBaseUrl: ap.assistantBaseUrl,
    schedule: ap.schedule,
    scheduleHour: ap.scheduleHour ?? 9,
    scheduleMinute: ap.scheduleMinute ?? 0,
  };
}
