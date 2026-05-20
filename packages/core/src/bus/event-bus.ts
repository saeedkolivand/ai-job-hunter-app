/**
 * Typed, in-process event bus.
 * All cross-runtime communication flows through this — never direct coupling.
 */
import type { JobEvent } from '@ajh/shared';

export interface EventMap {
  'job.event': JobEvent;
  'runtime.ready': { runtime: string };
  'runtime.error': { runtime: string; error: string };
  'state.changed': { key: string; value: unknown };
  // extend freely; keeps the bus typed end-to-end
  [key: `app.${string}`]: unknown;
}

export type EventHandler<T> = (payload: T) => void | Promise<void>;

export class EventBus {
  private readonly handlers = new Map<keyof EventMap | string, Set<EventHandler<unknown>>>();

  on<K extends keyof EventMap>(event: K, handler: EventHandler<EventMap[K]>): () => void;
  on(event: string, handler: EventHandler<unknown>): () => void;
  on(event: string, handler: EventHandler<unknown>): () => void {
    let set = this.handlers.get(event);
    if (!set) {
      set = new Set();
      this.handlers.set(event, set);
    }
    set.add(handler);
    const captured = set;
    return () => captured.delete(handler);
  }

  once<K extends keyof EventMap>(event: K, handler: EventHandler<EventMap[K]>): () => void {
    const off = this.on(event, async (payload) => {
      off();
      await handler(payload as EventMap[K]);
    });
    return off;
  }

  async emit<K extends keyof EventMap>(event: K, payload: EventMap[K]): Promise<void>;
  async emit(event: string, payload: unknown): Promise<void>;
  async emit(event: string, payload: unknown): Promise<void> {
    const set = this.handlers.get(event);
    if (!set) return;
    await Promise.all([...set].map((h) => Promise.resolve(h(payload)).catch(() => {})));
  }

  clear(): void {
    this.handlers.clear();
  }
}
