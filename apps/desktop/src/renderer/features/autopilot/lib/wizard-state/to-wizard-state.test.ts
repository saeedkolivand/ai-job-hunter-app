import { describe, expect, it } from 'vitest';

import { AGGREGATOR_BOARD_ID, type Autopilot, type JobPreferences } from '@ajh/shared';

import { autopilotToWizardState, buildDefaults, wizardStateToPayload } from '../wizard-state';

// ── Minimal valid Autopilot fixture ──────────────────────────────────────────

const BASE_AUTOPILOT: Autopilot = {
  _id: 'ap-1',
  name: 'My autopilot',
  status: 'active',
  target: {
    boards: ['linkedin'],
    query: 'react developer',
    location: 'Berlin',
    workTypes: ['remote'],
    pages: 2,
    dateFilter: '24h',
  },
  filter: {
    minMatchScore: 60,
    keywords: ['react', 'typescript'],
    excludeKeywords: ['senior'],
  },
  schedule: 'daily',
  scheduleHour: 7,
  scheduleMinute: 30,
  resumeText: 'My resume text',
  coverLetter: 'My cover letter',
  totalFound: 5,
  totalApplied: 1,
  createdAt: 1_700_000_000_000,
  updatedAt: 1_700_000_001_000,
};

// ── buildDefaults ─────────────────────────────────────────────────────────────

describe('buildDefaults()', () => {
  it('seeds scheduleHour: 9 and scheduleMinute: 0 with no prefs', () => {
    const state = buildDefaults();
    expect(state.scheduleHour).toBe(9);
    expect(state.scheduleMinute).toBe(0);
  });

  it('seeds scheduleHour: 9 and scheduleMinute: 0 when prefs are provided', () => {
    const state = buildDefaults({ location: 'Berlin' });
    expect(state.scheduleHour).toBe(9);
    expect(state.scheduleMinute).toBe(0);
  });

  it("defaults workTypes to an empty array (the 'any' sentinel; no job-preference seed)", () => {
    expect(buildDefaults().workTypes).toEqual([]);
    expect(buildDefaults({ location: 'Berlin' }).workTypes).toEqual([]);
  });

  it('sets default schedule to daily', () => {
    const state = buildDefaults();
    expect(state.schedule).toBe('daily');
  });

  it('pre-fills location from job preferences', () => {
    const state = buildDefaults({ location: 'Munich' });
    expect(state.location).toBe('Munich');
  });

  it('falls back to empty location when prefs are absent', () => {
    const state = buildDefaults();
    expect(state.location).toBe('');
  });

  it('pre-fills countryCode alongside location (autopilot aggregator zero-jobs fix)', () => {
    const state = buildDefaults({ location: 'Munich', countryCode: 'de' });
    expect(state.countryCode).toBe('de');
  });

  it('leaves countryCode undefined when prefs carry a location but no countryCode', () => {
    // A legacy JobPreferences record (saved before this fix) or a manually
    // typed preference has no countryCode — must not fabricate one.
    const state = buildDefaults({ location: 'Munich' });
    expect(state.countryCode).toBeUndefined();
  });

  it('defaults to boards: ["aggregator"]', () => {
    expect(buildDefaults().boards).toEqual(['aggregator']);
  });

  it('keywords is empty string even when jobPrefs has a techStack (Fix B regression lock)', () => {
    // Before the fix, keywords was seeded with the entire tech stack joined by ", ".
    // After: always '' so the must-include filter starts opt-in, not pre-populated.
    const prefs: JobPreferences = {
      techStack: [
        { name: 'react', category: 'frontend' },
        { name: 'typescript', category: 'language' },
      ],
    };
    expect(buildDefaults(prefs).keywords).toBe('');
  });

  it('dateFilter defaults to "" not "24h" (Fix C regression lock)', () => {
    // Pre-fix the wizard defaulted to '24h', which caused autopilot zero-jobs by
    // persisting a narrow date window. The default must now be '' (no filter).
    expect(buildDefaults().dateFilter).toBe('');
  });

  it('minMatchScore defaults to 0 not 50 (Fix C regression lock)', () => {
    // Pre-fix the default was 50, silently dropping most postings. Must be 0 so
    // every scraped posting is eligible unless the user raises the threshold.
    expect(buildDefaults().minMatchScore).toBe(0);
  });

  it('defaults assistant (Phase 4 AI notes) to false with no provider snapshot', () => {
    const state = buildDefaults();
    expect(state.assistant).toBe(false);
    expect(state.assistantProvider).toBeUndefined();
    expect(state.assistantModel).toBeUndefined();
    expect(state.assistantBaseUrl).toBeUndefined();
  });

  it('defaults watchedCompaniesOnly to false (ADR-030 §e)', () => {
    expect(buildDefaults().watchedCompaniesOnly).toBe(false);
  });

  it('defaults pages to 2, matching the backend AutopilotTargetSchema default', () => {
    // Was `amount: 50` (an item count the save step divided by 25). The form now
    // holds the page budget itself, so the default must be the page number.
    expect(buildDefaults().pages).toBe(2);
  });
});

// ── autopilotToWizardState ────────────────────────────────────────────────────

describe('autopilotToWizardState()', () => {
  it('round-trips scheduleHour and scheduleMinute from an Autopilot', () => {
    const state = autopilotToWizardState(BASE_AUTOPILOT);
    expect(state.scheduleHour).toBe(7);
    expect(state.scheduleMinute).toBe(30);
  });

  it('falls back to scheduleHour 9 when the field is absent on the Autopilot', () => {
    const ap: Autopilot = { ...BASE_AUTOPILOT, scheduleHour: undefined };
    const state = autopilotToWizardState(ap);
    expect(state.scheduleHour).toBe(9);
  });

  it('falls back to scheduleMinute 0 when the field is absent on the Autopilot', () => {
    const ap: Autopilot = { ...BASE_AUTOPILOT, scheduleMinute: undefined };
    const state = autopilotToWizardState(ap);
    expect(state.scheduleMinute).toBe(0);
  });

  it('maps both fields to 0 when the Autopilot explicitly stores 0', () => {
    const ap: Autopilot = { ...BASE_AUTOPILOT, scheduleHour: 0, scheduleMinute: 0 };
    const state = autopilotToWizardState(ap);
    expect(state.scheduleHour).toBe(0);
    expect(state.scheduleMinute).toBe(0);
  });

  it('round-trips all other top-level fields correctly', () => {
    const state = autopilotToWizardState(BASE_AUTOPILOT);
    expect(state.name).toBe('My autopilot');
    expect(state.boards).toEqual(['linkedin']);
    expect(state.query).toBe('react developer');
    expect(state.schedule).toBe('daily');
    expect(state.minMatchScore).toBe(60);
    expect(state.resumeText).toBe('My resume text');
  });

  it('reads all boards from a multi-board autopilot', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, boards: ['linkedin', 'indeed'] },
    };
    const state = autopilotToWizardState(ap);
    expect(state.boards).toEqual(['linkedin', 'indeed']);
  });

  it('falls back to aggregator when target.boards is empty', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, boards: [] },
    };
    const state = autopilotToWizardState(ap);
    expect(state.boards).toEqual([AGGREGATOR_BOARD_ID]);
  });

  it('round-trips countryCode when target carries one (Fix A)', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, countryCode: 'us' },
    };
    const state = autopilotToWizardState(ap);
    expect(state.countryCode).toBe('us');
  });

  it('yields undefined countryCode when target does not carry one', () => {
    // BASE_AUTOPILOT.target has no countryCode → wizard state must be undefined.
    const state = autopilotToWizardState(BASE_AUTOPILOT);
    expect(state.countryCode).toBeUndefined();
  });

  it('joins keywords array to a comma-separated string', () => {
    const state = autopilotToWizardState(BASE_AUTOPILOT);
    expect(state.keywords).toBe('react, typescript');
  });

  it('produces empty keywords string when keywords are absent', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      filter: { ...BASE_AUTOPILOT.filter, keywords: undefined },
    };
    const state = autopilotToWizardState(ap);
    expect(state.keywords).toBe('');
  });

  it('round-trips the assistant flag + provider snapshot (Phase 4 AI notes)', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      assistant: true,
      assistantProvider: 'anthropic',
      assistantModel: 'claude',
      assistantBaseUrl: undefined,
    };
    const state = autopilotToWizardState(ap);
    expect(state.assistant).toBe(true);
    expect(state.assistantProvider).toBe('anthropic');
    expect(state.assistantModel).toBe('claude');
    expect(state.assistantBaseUrl).toBeUndefined();
  });

  it('falls back to assistant: false when absent on the Autopilot (legacy record)', () => {
    const state = autopilotToWizardState(BASE_AUTOPILOT);
    expect(state.assistant).toBe(false);
  });

  it('round-trips a multi-value workTypes array from the target', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, workTypes: ['remote', 'hybrid'] },
    };
    expect(autopilotToWizardState(ap).workTypes).toEqual(['remote', 'hybrid']);
  });

  it('falls back to an empty workTypes array when target does not carry one (legacy record)', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, workTypes: undefined },
    };
    expect(autopilotToWizardState(ap).workTypes).toEqual([]);
  });

  it('round-trips watchedCompaniesOnly: true from the target (ADR-030 §e)', () => {
    const ap: Autopilot = {
      ...BASE_AUTOPILOT,
      target: { ...BASE_AUTOPILOT.target, watchedCompaniesOnly: true },
    };
    expect(autopilotToWizardState(ap).watchedCompaniesOnly).toBe(true);
  });

  it('falls back to watchedCompaniesOnly: false when absent (legacy record)', () => {
    expect(autopilotToWizardState(BASE_AUTOPILOT).watchedCompaniesOnly).toBe(false);
  });

  it('reads target.pages verbatim instead of the old ×25 item approximation', () => {
    // BASE_AUTOPILOT stores pages: 2 — the wizard used to surface that as 50
    // "items", which round-tripped back to 2 pages only because of the ÷25.
    expect(autopilotToWizardState(BASE_AUTOPILOT).pages).toBe(2);
  });

  it.each([1, 5, 10])('round-trips a stored pages: %i through the wizard unchanged', (pages) => {
    const ap: Autopilot = { ...BASE_AUTOPILOT, target: { ...BASE_AUTOPILOT.target, pages } };
    const state = autopilotToWizardState(ap);
    expect(state.pages).toBe(pages);
    expect(wizardStateToPayload(state).target.pages).toBe(pages);
  });
});
