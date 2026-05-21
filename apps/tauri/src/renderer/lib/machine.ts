/**
 * Minimal state machine utilities.
 *
 * No external library needed at this scale. A machine is just a typed
 * transition table. Guards and side-effects are kept in the hook that
 * uses the machine, keeping this layer pure.
 *
 * Usage:
 *   const machine = createMachine({ ... });
 *   const [state, send] = useMachine(machine, 'idle');
 */

export type Transition<TState extends string, TEvent extends string> = Partial<
  Record<TState, Partial<Record<TEvent, TState>>>
>;

export interface Machine<TState extends string, TEvent extends string> {
  transitions: Transition<TState, TEvent>;
  /** Optional: states where the machine is considered "busy" (loading indicator) */
  busyStates?: TState[];
  /** Optional: terminal error states */
  errorStates?: TState[];
}

export function createMachine<TState extends string, TEvent extends string>(
  def: Machine<TState, TEvent>
): Machine<TState, TEvent> {
  return def;
}

/**
 * Resolve the next state given current state + event.
 * Returns the current state unchanged if the transition is not defined
 * (implicit guard: undefined transitions are ignored).
 */
export function transition<TState extends string, TEvent extends string>(
  machine: Machine<TState, TEvent>,
  current: TState,
  event: TEvent
): TState {
  return machine.transitions[current]?.[event] ?? current;
}

export function isBusy<TState extends string, TEvent extends string>(
  machine: Machine<TState, TEvent>,
  state: TState
): boolean {
  return machine.busyStates?.includes(state) ?? false;
}

export function isError<TState extends string, TEvent extends string>(
  machine: Machine<TState, TEvent>,
  state: TState
): boolean {
  return machine.errorStates?.includes(state) ?? false;
}
