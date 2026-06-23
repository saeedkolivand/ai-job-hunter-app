import { describe, expect, it } from 'vitest';

import { AGGREGATOR_BOARD_ID, type Autopilot, type JobPreferences } from '@ajh/shared';

import type { WizardState } from '@/features/autopilot/types';

import {
  autopilotToWizardState,
  buildDefaults,
  itemsToPages,
  wizardStateToPayload,
} from './wizard-state';

// ── Minimal valid Autopilot fixture ──────────────────────────────────────────

const BASE_AUTOPILOT: Autopilot = {
  _id: 'ap-1',
  name: 'My autopilot',
  status: 'active',
  target: {
    boards: ['linkedin'],
    query: 'react developer',
    location: 'Berlin',
    workType: 'remote',
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

  it("defaults workType to the 'any' sentinel (no job-preference seed)", () => {
    expect(buildDefaults().workType).toBe('any');
    expect(buildDefaults({ location: 'Berlin' }).workType).toBe('any');
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
});

// ── wizardStateToPayload ──────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Backend roles',
    boards: ['linkedin'],
    query: 'rust backend',
    location: 'Berlin',
    workType: 'remote',
    amount: 75,
    dateFilter: '24h',
    minMatchScore: 70,
    keywords: 'rust, tokio',
    excludeKeywords: 'php',
    resumeText: 'my resume',
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 30,
    ...overrides,
  };
}

describe('wizardStateToPayload()', () => {
  it('maps a fully-populated form onto the create payload', () => {
    expect(wizardStateToPayload(makeForm())).toEqual({
      name: 'Backend roles',
      target: {
        boards: ['linkedin'],
        query: 'rust backend',
        location: 'Berlin',
        workType: 'remote',
        pages: 3,
        dateFilter: '24h',
      },
      filter: {
        minMatchScore: 70,
        keywords: ['rust', 'tokio'],
        excludeKeywords: ['php'],
      },
      resumeText: 'my resume',
      schedule: 'daily',
      scheduleHour: 9,
      scheduleMinute: 30,
    });
  });

  describe('keyword splitting', () => {
    it('trims, splits on comma, and drops empty fragments', () => {
      expect(
        wizardStateToPayload(makeForm({ keywords: ' rust ,, tokio , ' })).filter.keywords
      ).toEqual(['rust', 'tokio']);
    });

    it('maps an empty / whitespace-only / comma-only string to undefined', () => {
      expect(wizardStateToPayload(makeForm({ keywords: '' })).filter.keywords).toBeUndefined();
      expect(wizardStateToPayload(makeForm({ keywords: '   ' })).filter.keywords).toBeUndefined();
      expect(wizardStateToPayload(makeForm({ keywords: ', ,' })).filter.keywords).toBeUndefined();
    });

    it('applies the same rule to excludeKeywords', () => {
      expect(
        wizardStateToPayload(makeForm({ excludeKeywords: '' })).filter.excludeKeywords
      ).toBeUndefined();
      expect(
        wizardStateToPayload(makeForm({ excludeKeywords: 'java, c#' })).filter.excludeKeywords
      ).toEqual(['java', 'c#']);
    });
  });

  describe('items → pages conversion', () => {
    it.each([
      [1, 1],
      [25, 1],
      [26, 2],
      [50, 2],
      [75, 3],
      [250, 10],
      [500, 10], // clamped to the backend max of 10 pages
    ])('maps amount %i to %i page(s)', (amount, pages) => {
      expect(itemsToPages(amount)).toBe(pages);
      expect(wizardStateToPayload(makeForm({ amount })).target.pages).toBe(pages);
    });

    it('falls back to one page for non-positive / non-finite amounts', () => {
      expect(itemsToPages(0)).toBe(1);
      expect(itemsToPages(-5)).toBe(1);
      expect(itemsToPages(Number.NaN)).toBe(1);
    });
  });

  describe('workType sentinel', () => {
    it("drops workType when it is the 'any' sentinel", () => {
      expect(wizardStateToPayload(makeForm({ workType: 'any' })).target.workType).toBeUndefined();
    });

    it('keeps a concrete workType', () => {
      expect(wizardStateToPayload(makeForm({ workType: 'hybrid' })).target.workType).toBe('hybrid');
    });
  });

  describe('schedule time', () => {
    it('drops hour/minute for a manual schedule', () => {
      const payload = wizardStateToPayload(makeForm({ schedule: 'manual' }));
      expect(payload.scheduleHour).toBeUndefined();
      expect(payload.scheduleMinute).toBeUndefined();
    });

    it('keeps hour/minute for recurring schedules', () => {
      const payload = wizardStateToPayload(
        makeForm({ schedule: 'twice_daily', scheduleHour: 6, scheduleMinute: 15 })
      );
      expect(payload.scheduleHour).toBe(6);
      expect(payload.scheduleMinute).toBe(15);
    });
  });

  describe('empty optionals collapse to undefined', () => {
    it('drops empty location, dateFilter, and resumeText', () => {
      const payload = wizardStateToPayload(
        makeForm({ location: '', dateFilter: '', resumeText: '' })
      );
      expect(payload.target.location).toBeUndefined();
      expect(payload.target.dateFilter).toBeUndefined();
      expect(payload.resumeText).toBeUndefined();
    });

    it('maps dateFilter: "" → undefined in target (Fix C regression lock)', () => {
      // The new buildDefaults() seeds '' (not '24h'). This confirms '' collapses
      // to undefined on the wire so no date restriction is forwarded to the scraper.
      const payload = wizardStateToPayload(makeForm({ dateFilter: '' }));
      expect(payload.target.dateFilter).toBeUndefined();
    });

    it('passes minMatchScore: 0 through to filter.minMatchScore (Fix C regression lock)', () => {
      // 0 is a legitimate "keep everything" threshold; it must not be treated as
      // falsy and dropped. The payload must carry 0 as-is.
      const payload = wizardStateToPayload(makeForm({ minMatchScore: 0 }));
      expect(payload.filter.minMatchScore).toBe(0);
    });
  });

  describe('countryCode forwarding (Fix A)', () => {
    it('forwards a non-empty countryCode from the form to target', () => {
      const payload = wizardStateToPayload(makeForm({ countryCode: 'gb' }));
      expect(payload.target.countryCode).toBe('gb');
    });

    it('drops countryCode when it is undefined in the form', () => {
      const payload = wizardStateToPayload(makeForm({ countryCode: undefined }));
      expect(payload.target.countryCode).toBeUndefined();
    });

    it('drops countryCode when it is an empty string in the form', () => {
      // The || undefined guard collapses '' to undefined so it is not forwarded.
      const payload = wizardStateToPayload(makeForm({ countryCode: '' }));
      expect(payload.target.countryCode).toBeUndefined();
    });
  });
});
