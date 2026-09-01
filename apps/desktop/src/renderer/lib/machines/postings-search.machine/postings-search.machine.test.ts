import { describe, expect, it } from 'vitest';

import { isBusy, isError, transition } from '@/lib/machine';

import { postingsSearchMachine } from './postings-search.machine';

describe('postingsSearchMachine', () => {
  it('walks the happy path to a hit', () => {
    let s = transition(postingsSearchMachine, 'idle', 'SUBMIT');
    expect(s).toBe('searching');
    s = transition(postingsSearchMachine, s, 'SETTLED_RESULTS');
    expect(s).toBe('results');
  });

  it('distinguishes zero hits from a genuine error', () => {
    expect(transition(postingsSearchMachine, 'searching', 'SETTLED_EMPTY')).toBe('noResults');
    expect(transition(postingsSearchMachine, 'searching', 'FAILED')).toBe('error');
    expect(transition(postingsSearchMachine, 'searching', 'SETTLED_STALE')).toBe('stale');
  });

  it('lets SUBMIT re-issue the query from every settled state', () => {
    for (const state of ['results', 'noResults', 'stale', 'error'] as const) {
      expect(transition(postingsSearchMachine, state, 'SUBMIT')).toBe('searching');
    }
  });

  it('CLEAR always returns to idle', () => {
    for (const state of ['searching', 'results', 'noResults', 'stale', 'error'] as const) {
      expect(transition(postingsSearchMachine, state, 'CLEAR')).toBe('idle');
    }
  });

  it('ignores an undefined transition instead of throwing (no CLEAR-in-idle no-op crash)', () => {
    expect(transition(postingsSearchMachine, 'idle', 'CLEAR')).toBe('idle');
  });

  it('classifies busy and error states', () => {
    expect(isBusy(postingsSearchMachine, 'searching')).toBe(true);
    expect(isBusy(postingsSearchMachine, 'results')).toBe(false);
    expect(isError(postingsSearchMachine, 'error')).toBe(true);
    expect(isError(postingsSearchMachine, 'noResults')).toBe(false);
  });
});
