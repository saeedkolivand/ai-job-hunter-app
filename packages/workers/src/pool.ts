/**
 * Reusable worker_thread pool with adaptive concurrency.
 *
 * Each worker script accepts a single message (`{ id, payload }`) and replies
 * with `{ id, ok, result | error }`. Workers stay warm and process tasks until
 * the pool is destroyed.
 */
import { cpus, freemem, totalmem } from 'node:os';
import { Worker } from 'node:worker_threads';

import { createLogger } from '@ajh/core';

export interface PoolOptions {
  scriptPath: string; // absolute path to compiled worker file
  minWorkers?: number;
  maxWorkers?: number;
  memoryFloorMB?: number; // never spawn new workers below this free mem
  idleTimeoutMs?: number;
  maxQueueSize?: number; // reject new tasks when pending queue exceeds this (0 = unlimited)
}

export interface WorkerTask<TPayload = unknown, _TResult = unknown> {
  payload: TPayload;
  signal?: AbortSignal;
}

interface Pending {
  id: string;
  payload: unknown;
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
}

interface Slot {
  worker: Worker;
  busy: boolean;
  lastUsed: number;
  current?: Pending;
}

export class WorkerPool {
  private readonly logger = createLogger('worker-pool');
  private readonly slots: Slot[] = [];
  private readonly queue: Pending[] = [];
  private readonly opts: Required<PoolOptions>;
  private nextId = 0;
  private destroyed = false;

  constructor(opts: PoolOptions) {
    const cpuCount = cpus().length;
    this.opts = {
      scriptPath: opts.scriptPath,
      minWorkers: opts.minWorkers ?? 1,
      maxWorkers: opts.maxWorkers ?? Math.max(2, Math.min(cpuCount - 1, 6)),
      memoryFloorMB: opts.memoryFloorMB ?? 512,
      idleTimeoutMs: opts.idleTimeoutMs ?? 60_000,
      maxQueueSize: opts.maxQueueSize ?? 0,
    };
    for (let i = 0; i < this.opts.minWorkers; i++) this.spawn();
    setInterval(() => this.gc(), 30_000).unref?.();
  }

  get queueLength(): number {
    return this.queue.length;
  }

  get activeWorkers(): number {
    return this.slots.filter((s) => s.busy).length;
  }

  async run<TPayload, TResult>(task: WorkerTask<TPayload, TResult>): Promise<TResult> {
    if (this.destroyed) throw new Error('Worker pool destroyed');
    if (this.opts.maxQueueSize > 0 && this.queue.length >= this.opts.maxQueueSize) {
      this.logger.warn(
        { queueLength: this.queue.length, maxQueueSize: this.opts.maxQueueSize },
        'worker pool queue full'
      );
      throw new Error(`Worker pool queue full (max ${this.opts.maxQueueSize})`);
    }
    return new Promise<TResult>((resolve, reject) => {
      const pending: Pending = {
        id: String(++this.nextId),
        payload: task.payload,
        resolve: resolve as (v: unknown) => void,
        reject,
      };
      if (task.signal) {
        if (task.signal.aborted) return reject(new Error('aborted'));
        task.signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
      }
      this.queue.push(pending);
      this.dispatch();
    });
  }

  destroy(): void {
    this.destroyed = true;
    for (const s of this.slots) s.worker.terminate();
    this.slots.length = 0;
  }

  private dispatch(): void {
    while (this.queue.length > 0) {
      const slot = this.slots.find((s) => !s.busy);
      if (!slot) {
        if (this.canSpawn()) this.spawn();
        else break;
        continue;
      }
      const task = this.queue.shift();
      if (!task) break;
      slot.busy = true;
      slot.current = task;
      slot.worker.postMessage({ id: task.id, payload: task.payload });
    }
  }

  private canSpawn(): boolean {
    if (this.slots.length >= this.opts.maxWorkers) return false;
    const freeMB = freemem() / 1024 / 1024;
    if (freeMB < this.opts.memoryFloorMB) {
      this.logger.warn({ freeMB, totalMB: totalmem() / 1024 / 1024 }, 'memory floor reached');
      return false;
    }
    return true;
  }

  private spawn(): Slot {
    const worker = new Worker(this.opts.scriptPath);
    const slot: Slot = { worker, busy: false, lastUsed: Date.now() };
    worker.on('message', (msg: { id: string; ok: boolean; result?: unknown; error?: string }) => {
      const cur = slot.current;
      slot.current = undefined;
      slot.busy = false;
      slot.lastUsed = Date.now();
      if (cur) {
        if (msg.ok) cur.resolve(msg.result);
        else cur.reject(new Error(msg.error ?? 'worker error'));
      }
      this.dispatch();
    });
    worker.on('error', (err) => {
      this.logger.error({ err }, 'worker error');
      slot.current?.reject(err as Error);
      this.replace(slot);
    });
    worker.on('exit', (code) => {
      if (code !== 0 && !this.destroyed) this.replace(slot);
    });
    this.slots.push(slot);
    return slot;
  }

  private replace(slot: Slot): void {
    const idx = this.slots.indexOf(slot);
    if (idx >= 0) this.slots.splice(idx, 1);
    if (!this.destroyed && this.slots.length < this.opts.minWorkers) this.spawn();
  }

  private gc(): void {
    const now = Date.now();
    while (this.slots.length > this.opts.minWorkers) {
      const idleIdx = this.slots.findIndex(
        (s) => !s.busy && now - s.lastUsed > this.opts.idleTimeoutMs
      );
      if (idleIdx < 0) break;
      const [slot] = this.slots.splice(idleIdx, 1);
      slot?.worker.terminate();
    }
  }
}
