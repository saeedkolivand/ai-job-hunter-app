import {
  AGGREGATOR_BOARD_ID,
  type Autopilot,
  type AutopilotCreate,
  type JobPreferences,
} from '@ajh/shared';

import type { WizardState } from '@/features/autopilot/types';

/** Scrapers paginate in ~25-result pages. */
const PAGE_SIZE = 25;
/** Backend cap on autopilot scraper pages (AutopilotTargetSchema.pages: max 10). */
const MAX_PAGES = 10;

/**
 * Convert a requested job count into a scraper page budget, clamped to the
 * backend's allowed range — mirrors the jobs page's `jobsToPages` (#41).
 */
export function itemsToPages(amount: number): number {
  const n = Number.isFinite(amount) && amount > 0 ? amount : PAGE_SIZE;
  return Math.min(Math.max(Math.ceil(n / PAGE_SIZE), 1), MAX_PAGES);
}

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
      workType: form.workType !== 'any' ? form.workType : undefined,
      pages: itemsToPages(form.amount),
      dateFilter: form.dateFilter || undefined,
    },
    filter: {
      minMatchScore: form.minMatchScore,
      keywords: splitKeywords(form.keywords),
      excludeKeywords: splitKeywords(form.excludeKeywords),
    },
    resumeText: form.resumeText || undefined,
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
    // No job-preference field seeds work type; default to the 'any' sentinel.
    workType: 'any',
    amount: 50,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
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
    workType: target.workType ?? 'any',
    // Stored as pages; surface back as an approximate item count for editing.
    amount: target.pages * PAGE_SIZE,
    dateFilter: target.dateFilter ?? '',
    minMatchScore: ap.filter.minMatchScore,
    keywords: ap.filter.keywords?.join(', ') ?? '',
    excludeKeywords: ap.filter.excludeKeywords?.join(', ') ?? '',
    resumeText: ap.resumeText ?? '',
    schedule: ap.schedule,
    scheduleHour: ap.scheduleHour ?? 9,
    scheduleMinute: ap.scheduleMinute ?? 0,
  };
}
