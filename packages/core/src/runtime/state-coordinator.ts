/**
 * State Coordinator — a small main-side reactive store for cross-runtime
 * shared state (selected locale, active model, runtime health snapshots).
 * Mirrors changes to the renderer through the EventBus.
 */
import type { EventBus } from '../bus/event-bus.js';

export class StateCoordinator<TState extends Record<string, unknown> = Record<string, unknown>> {
  private state: TState;

  constructor(
    private readonly bus: EventBus,
    initial: TState
  ) {
    this.state = { ...initial };
  }

  get<K extends keyof TState>(key: K): TState[K] {
    return this.state[key];
  }

  snapshot(): TState {
    return { ...this.state };
  }

  async set<K extends keyof TState>(key: K, value: TState[K]): Promise<void> {
    this.state[key] = value;
    await this.bus.emit('state.changed', { key: String(key), value });
  }

  async patch(patch: Partial<TState>): Promise<void> {
    for (const [k, v] of Object.entries(patch)) {
      await this.set(k as keyof TState, v as TState[keyof TState]);
    }
  }
}
