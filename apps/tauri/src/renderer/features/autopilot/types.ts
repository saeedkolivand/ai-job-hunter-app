import type { AutopilotSchedule } from '@ajh/shared';

export interface WizardState {
  name: string;
  // Step 1 — Target
  board: string;
  query: string;
  location: string;
  workType: 'remote' | 'hybrid' | 'on-site' | 'any';
  pages: number;
  dateFilter: string;
  // Step 2 — Filter
  minMatchScore: number;
  keywords: string;
  excludeKeywords: string;
  resumeText: string;
  // Step 3 — Apply assistant (optional base cover letter the assistant tailors)
  coverLetter: string;
  // Step 4 — Schedule
  schedule: AutopilotSchedule;
  /** Local clock hour (0–23) recurring schedules fire at. Used by daily/twice_daily. */
  scheduleHour: number;
  /** Local clock minute (0–59). Used by daily/twice_daily and as "minute past the hour" for hourly. */
  scheduleMinute: number;
}

export type SetFn = <K extends keyof WizardState>(k: K, v: WizardState[K]) => void;
export type Prefilled = { location: boolean; keywords: boolean };
