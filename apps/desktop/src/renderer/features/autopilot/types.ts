import type { AutopilotSchedule } from '@ajh/shared';

export interface WizardState {
  name: string;
  // Step 1 — Target
  boards: string[];
  query: string;
  location: string;
  /** Country code captured when the user picks a geocode suggestion (e.g. "gb", "us"). */
  countryCode?: string;
  workType: 'remote' | 'hybrid' | 'on-site' | 'any';
  /** Target number of jobs to fetch; converted to scraper pages on save (mirrors the jobs page). */
  amount: number;
  dateFilter: string;
  // Step 2 — Filter
  minMatchScore: number;
  keywords: string;
  excludeKeywords: string;
  resumeText: string;
  // Step 3 — Action
  /** Opt-in (Phase 4): ask for a short AI-reasoned note on the top matches of
   *  each scheduled run. The scheduler runs headless (no renderer), so the
   *  active provider is snapshotted into the fields below when this is on. */
  assistant: boolean;
  assistantProvider?: string;
  assistantModel?: string;
  assistantBaseUrl?: string;
  // Step 4 — Schedule
  schedule: AutopilotSchedule;
  /** Local clock hour (0–23) recurring schedules fire at. Used by daily/twice_daily. */
  scheduleHour: number;
  /** Local clock minute (0–59). Used by daily/twice_daily and as "minute past the hour" for hourly. */
  scheduleMinute: number;
}

export type Prefilled = { location: boolean };
