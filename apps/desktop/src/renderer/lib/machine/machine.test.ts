import { describe, expect, it } from 'vitest';

import { createMachine, isBusy, isError, transition } from './machine';

type S = 'idle' | 'loading' | 'success' | 'failure';
type E = 'FETCH' | 'RESOLVE' | 'REJECT' | 'RESET';

const machine = createMachine<S, E>({
  transitions: {
    idle: { FETCH: 'loading' },
    loading: { RESOLVE: 'success', REJECT: 'failure' },
    success: { RESET: 'idle' },
    failure: { RESET: 'idle', FETCH: 'loading' },
  },
  busyStates: ['loading'],
  errorStates: ['failure'],
});

describe('state machine', () => {
  it('follows defined transitions', () => {
    expect(transition(machine, 'idle', 'FETCH')).toBe('loading');
    expect(transition(machine, 'loading', 'RESOLVE')).toBe('success');
    expect(transition(machine, 'loading', 'REJECT')).toBe('failure');
    expect(transition(machine, 'failure', 'RESET')).toBe('idle');
  });

  it('stays put for undefined transitions', () => {
    expect(transition(machine, 'idle', 'RESOLVE')).toBe('idle');
    expect(transition(machine, 'success', 'FETCH')).toBe('success');
  });

  it('reports busy states', () => {
    expect(isBusy(machine, 'loading')).toBe(true);
    expect(isBusy(machine, 'idle')).toBe(false);
  });

  it('reports error states', () => {
    expect(isError(machine, 'failure')).toBe(true);
    expect(isError(machine, 'success')).toBe(false);
  });

  it('defaults busy/error to false when not configured', () => {
    const bare = createMachine<S, E>({ transitions: {} });
    expect(isBusy(bare, 'loading')).toBe(false);
    expect(isError(bare, 'failure')).toBe(false);
  });
});
