import { describe, expect, it } from 'vitest';

import type { Autopilot } from '@ajh/shared';

import { autopilotToWizardState, buildDefaults } from './wizard-state';

// ── Minimal valid Autopilot fixture ──────────────────────────────────────────

const BASE_AUTOPILOT: Autopilot = {
  _id: 'ap-1',
  name: 'My autopilot',
  status: 'active',
  target: {
    board: 'linkedin',
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
    const state = buildDefaults({ location: 'Berlin', remote: 'hybrid' });
    expect(state.scheduleHour).toBe(9);
    expect(state.scheduleMinute).toBe(0);
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
    expect(state.board).toBe('linkedin');
    expect(state.query).toBe('react developer');
    expect(state.schedule).toBe('daily');
    expect(state.minMatchScore).toBe(60);
    expect(state.resumeText).toBe('My resume text');
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
