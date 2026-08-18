import { describe, expect, it } from 'vitest';

import { isBusy, isError, transition } from '@/lib/machine';

import { autopilotRunMachine, RUN_STATE_LABEL, stepToEvent } from './autopilot-run.machine';

describe('autopilotRunMachine', () => {
  it('progresses through the find-and-rank lifecycle', () => {
    let s = transition(autopilotRunMachine, 'idle', 'START');
    expect(s).toBe('scraping');
    s = transition(autopilotRunMachine, s, 'SCRAPE_DONE');
    expect(s).toBe('ranking');
    s = transition(autopilotRunMachine, s, 'COMPLETE');
    expect(s).toBe('done');
  });

  it('supports cancellation and error from busy states', () => {
    expect(transition(autopilotRunMachine, 'scraping', 'CANCEL')).toBe('cancelled');
    expect(transition(autopilotRunMachine, 'ranking', 'ERROR')).toBe('error');
    expect(isBusy(autopilotRunMachine, 'ranking')).toBe(true);
    expect(isError(autopilotRunMachine, 'error')).toBe(true);
  });

  it('maps backend step strings to events', () => {
    expect(stepToEvent('scrape_start')).toBe('SCRAPE_START');
    expect(stepToEvent('rank_done')).toBe('RANK_DONE');
    expect(stepToEvent('complete')).toBe('COMPLETE');
    expect(stepToEvent('cancelled')).toBe('CANCEL');
    expect(stepToEvent('unknown-step')).toBeNull();
  });

  it('accepts every mid-run observation from idle, since idle also means "not seen yet"', () => {
    // The run lives in the backend; `runStates` is component-local and empty
    // after any navigation. A card that rejoins mid-run gets these WITHOUT a
    // preceding SCRAPE_START, and dropping them left it claiming idle while the
    // backend refused a second run as already in progress.
    expect(transition(autopilotRunMachine, 'idle', 'SCRAPE_DONE')).toBe('ranking');
    expect(transition(autopilotRunMachine, 'idle', 'RANK_DONE')).toBe('ranking');
    expect(transition(autopilotRunMachine, 'idle', 'COMPLETE')).toBe('done');
    expect(transition(autopilotRunMachine, 'idle', 'CANCEL')).toBe('cancelled');
    expect(transition(autopilotRunMachine, 'idle', 'ERROR')).toBe('error');
  });

  it('re-arms from a terminal state when the NEXT run announces itself', () => {
    // Terminal is terminal for one run, not for the autopilot. The scheduler's
    // next occurrence (or a run started elsewhere) opens with scrape_start, and
    // without these the card would sit on the previous outcome while a new run
    // streamed underneath it.
    expect(transition(autopilotRunMachine, 'done', 'SCRAPE_START')).toBe('scraping');
    expect(transition(autopilotRunMachine, 'cancelled', 'SCRAPE_START')).toBe('scraping');
    expect(transition(autopilotRunMachine, 'error', 'SCRAPE_START')).toBe('scraping');
    // Still terminal for everything else — a stray late COMPLETE from the run
    // that already ended must not resurrect it into a new lifecycle.
    expect(transition(autopilotRunMachine, 'done', 'COMPLETE')).toBe('done');
    expect(transition(autopilotRunMachine, 'cancelled', 'SCRAPE_DONE')).toBe('cancelled');
  });

  it('provides a label for every state', () => {
    expect(RUN_STATE_LABEL.scraping).toBe('Scraping…');
    expect(RUN_STATE_LABEL.done).toBe('Done');
    expect(Object.keys(RUN_STATE_LABEL)).toHaveLength(6);
  });
});
