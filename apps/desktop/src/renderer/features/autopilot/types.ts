import type { AutopilotSchedule, WorkTypeOption } from '@ajh/shared';

export interface WizardState {
  name: string;
  // Step 1 — Target
  boards: string[];
  query: string;
  location: string;
  /** Country code captured when the user picks a geocode suggestion (e.g. "gb", "us"). */
  countryCode?: string;
  /** Requested work arrangement(s); empty = any (mirrors the manual search form). */
  workTypes: WorkTypeOption[];
  /**
   * Scraper page budget (integer 1–10) — stored and sent verbatim as `target.pages`.
   * Each board decides what a "page" means (LinkedIn 10 results, The Muse 20) and
   * some ignore it entirely, so this is a ceiling per board, not a job count.
   */
  pages: number;
  dateFilter: string;
  /**
   * Watched-companies-only mode (ADR-030 §e): when true, the run resolves the
   * user's currently-starred discovered companies at run time instead of the
   * curated seed. Additive + optional so old autopilots load unchanged.
   */
  watchedCompaniesOnly: boolean;
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
