import { describe, expect, it } from 'vitest';

import { isBusy, isError, transition } from '@/lib/machine';

import { autopilotRunMachine, RUN_STATE_LABEL, stepToEvent } from './autopilot-run.machine';

describe('autopilotRunMachine', () => {
  it('progresses through the run lifecycle', () => {
    let s = transition(autopilotRunMachine, 'idle', 'START');
    expect(s).toBe('scraping');
    s = transition(autopilotRunMachine, s, 'SCRAPE_DONE');
    expect(s).toBe('ranking');
    s = transition(autopilotRunMachine, s, 'APPLY_START');
    expect(s).toBe('applying');
    s = transition(autopilotRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });

  it('supports cancellation and error from busy states', () => {
    expect(transition(autopilotRunMachine, 'scraping', 'CANCEL')).toBe('cancelled');
    expect(transition(autopilotRunMachine, 'applying', 'ERROR')).toBe('error');
    expect(isBusy(autopilotRunMachine, 'applying')).toBe(true);
    expect(isError(autopilotRunMachine, 'error')).toBe(true);
  });

  it('maps backend step strings to events', () => {
    expect(stepToEvent('scrape_start')).toBe('SCRAPE_START');
    expect(stepToEvent('rank_done')).toBe('RANK_DONE');
    expect(stepToEvent('complete')).toBe('COMPLETE');
    expect(stepToEvent('cancelled')).toBe('CANCEL');
    expect(stepToEvent('unknown-step')).toBeNull();
  });

  it('provides a label for every state', () => {
    expect(RUN_STATE_LABEL.scraping).toBe('Scraping…');
    expect(RUN_STATE_LABEL.done).toBe('Done');
    expect(Object.keys(RUN_STATE_LABEL)).toHaveLength(7);
  });
});
