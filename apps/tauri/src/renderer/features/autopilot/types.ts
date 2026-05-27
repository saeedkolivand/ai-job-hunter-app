import type { AutopilotAction, AutopilotSchedule } from '@ajh/shared';

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
  // Step 3 — Action
  action: AutopilotAction;
  coverLetter: string;
  autoSubmit: boolean;
  // Step 4 — Schedule
  schedule: AutopilotSchedule;
}

export type SetFn = <K extends keyof WizardState>(k: K, v: WizardState[K]) => void;
export type Prefilled = { location: boolean; keywords: boolean };
