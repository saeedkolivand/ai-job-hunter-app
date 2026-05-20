/**
 * Runtime Manager — owns the lifecycle of AI Runtime, Data Runtime, and
 * any future runtimes. Starts them lazily, monitors health, coordinates
 * graceful shutdown.
 */
import type { EventBus } from '../bus/event-bus.js';
import { createLogger, type Logger } from '../logger.js';

export interface Runtime {
  readonly id: string;
  start(): Promise<void>;
  stop(): Promise<void>;
  health(): Promise<Record<string, unknown>>;
}

export class RuntimeManager {
  private readonly runtimes = new Map<string, Runtime>();
  private readonly started = new Set<string>();
  private readonly logger: Logger = createLogger('runtime-manager');

  constructor(private readonly bus: EventBus) {}

  register(runtime: Runtime): void {
    this.runtimes.set(runtime.id, runtime);
  }

  async start(id?: string): Promise<void> {
    const targets = id
      ? ([this.runtimes.get(id)].filter(Boolean) as Runtime[])
      : [...this.runtimes.values()];
    for (const rt of targets) {
      if (this.started.has(rt.id)) continue;
      try {
        await rt.start();
        this.started.add(rt.id);
        await this.bus.emit('runtime.ready', { runtime: rt.id });
        this.logger.info({ runtime: rt.id, id: rt.id }, 'runtime started');
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        this.logger.error({ runtime: rt.id, err: message }, 'runtime failed to start');
        await this.bus.emit('runtime.error', { runtime: rt.id, error: message });
      }
    }
  }

  async stop(): Promise<void> {
    for (const rt of this.runtimes.values()) {
      if (!this.started.has(rt.id)) continue;
      try {
        await rt.stop();
      } catch (err) {
        this.logger.error({ runtime: rt.id, err }, 'runtime failed to stop');
      }
    }
    this.started.clear();
  }

  get<T extends Runtime = Runtime>(id: string): T | undefined {
    return this.runtimes.get(id) as T | undefined;
  }

  async health(): Promise<Record<string, Record<string, unknown>>> {
    const out: Record<string, Record<string, unknown>> = {};
    for (const rt of this.runtimes.values()) {
      try {
        out[rt.id] = await rt.health();
      } catch {
        out[rt.id] = { ready: false };
      }
    }
    return out;
  }
}
