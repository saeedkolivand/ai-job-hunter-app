/**
 * AI Runtime — implements core.Runtime.
 *
 * Responsibilities:
 *  - Ollama orchestration
 *  - streaming AI
 *  - embeddings generation
 *  - model lifecycle (load on demand, unload on idle)
 *  - memory monitoring
 *
 * Heavy work is dispatched through the EventBus and JobQueue. The runtime
 * does NOT register its own IPC handlers — the main process wires those.
 */

import { createLogger, type Runtime, type EventBus } from '@ajh/core';
import { OllamaClient } from './client/ollama.js';
import type { ModelInfo } from './client/models.js';

export interface AiRuntimeOptions {
  host?: string;
  idleUnloadMs?: number; // unload models after this idle window
}

export class AiRuntime implements Runtime {
  readonly id = 'ai';
  private readonly logger = createLogger('runtime.ai');
  private readonly client: OllamaClient;
  private readonly loaded = new Map<string, ModelInfo>();
  private readonly idleUnloadMs: number;
  private idleTimer?: ReturnType<typeof setInterval>;

  constructor(
    private readonly bus: EventBus,
    opts: AiRuntimeOptions = {}
  ) {
    this.client = new OllamaClient({ ...(opts.host ? { host: opts.host } : {}) });
    this.idleUnloadMs = opts.idleUnloadMs ?? 10 * 60_000;
  }

  async start(): Promise<void> {
    try {
      const models = await this.client.listModels();
      this.logger.info({ models: models.length, modelList: models }, 'ollama reachable');
    } catch (err) {
      this.logger.warn(
        { err },
        'ollama not reachable — AI features will be unavailable until it starts'
      );
    }
    this.idleTimer = setInterval(() => this.unloadIdle(), 60_000);
  }

  async stop(): Promise<void> {
    if (this.idleTimer) clearInterval(this.idleTimer);
    for (const m of this.loaded.keys()) {
      try {
        await this.client.unload(m);
      } catch {
        /* noop */
      }
    }
    this.loaded.clear();
  }

  async health(): Promise<Record<string, unknown>> {
    try {
      const models = await this.client.listModels();
      return { ready: true, models, loaded: [...this.loaded.keys()] };
    } catch {
      return { ready: false };
    }
  }

  getClient(): OllamaClient {
    return this.client;
  }

  markUsed(model: string, kind: ModelInfo['kind'], dimensions?: number): void {
    const info = this.loaded.get(model) ?? { name: model, kind, loadedAt: Date.now() };
    info.lastUsedAt = Date.now();
    if (dimensions !== undefined) info.dimensions = dimensions;
    this.loaded.set(model, info);
  }

  private async unloadIdle(): Promise<void> {
    const now = Date.now();
    for (const [name, info] of this.loaded) {
      if (info.lastUsedAt && now - info.lastUsedAt > this.idleUnloadMs) {
        try {
          await this.client.unload(name);
          this.loaded.delete(name);
          this.logger.info({ model: name }, 'unloaded idle model');
        } catch (err) {
          this.logger.warn({ err, model: name }, 'failed to unload model');
        }
      }
    }
  }
}
