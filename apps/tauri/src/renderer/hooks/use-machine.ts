import { useCallback, useState } from 'react';

import { isBusy, isError, type Machine, transition } from '@/lib/machine';

/**
 * React hook for driving a state machine.
 *
 * Usage:
 *   const [state, send] = useMachine(aiGenerateMachine, 'idle');
 *   send('START');   // transitions to next state per machine definition
 */
export function useMachine<TState extends string, TEvent extends string>(
  machine: Machine<TState, TEvent>,
  initial: TState
) {
  const [state, setState] = useState<TState>(initial);

  const send = useCallback(
    (event: TEvent) => setState((current) => transition(machine, current, event)),
    [machine]
  );

  return [state, send, { busy: isBusy(machine, state), error: isError(machine, state) }] as const;
}
