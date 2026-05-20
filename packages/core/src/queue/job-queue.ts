/**
 * Unified job system.
 *
 * Every heavy operation (AI, OCR, scrape, embed, import, index, match)
 * goes through this queue. Jobs are persistent (via a pluggable persistor),
 * support cancellation, retries, progress, and streaming events.
 */
import { nanoid } from 'nanoid';

import type { JobEvent, JobKind, JobRecord, JobStatus } from '@ajh/shared';

import type { EventBus } from '../bus/event-bus.js';
import { createLogger, type Logger } from '../logger.js';

export interface EnqueueOptions {
  maxRetries?: number;
  priority?: number; // higher runs first
}

export interface JobContext<TPayload> {
  job: JobRecord<TPayload>;
  signal: AbortSignal;
  setProgress(p: number): void;
  stream(delta: unknown): void;
  logger: Logger;
}

export type JobHandler<TPayload = unknown, TResult = unknown> = (
  ctx: JobContext<TPayload>
) => Promise<TResult>;

export interface JobPersistor {
  save(job: JobRecord): Promise<void>;
  load(): Promise<JobRecord[]>;
  remove(jobId: string): Promise<void>;
}

export interface JobQueueOptions {
  concurrency?: number;
  persistor?: JobPersistor;
}

export class JobQueue {
  private readonly handlers = new Map<JobKind, JobHandler<unknown, unknown>>();
  private readonly jobs = new Map<string, JobRecord>();
  private readonly aborts = new Map<string, AbortController>();
  private readonly pending: string[] = [];
  private running = 0;
  private concurrency: number;
  private readonly persistor?: JobPersistor;
  private readonly logger: Logger;

  constructor(
    private readonly bus: EventBus,
    opts: JobQueueOptions = {}
  ) {
    this.concurrency = opts.concurrency ?? 4;
    this.persistor = opts.persistor;
    this.logger = createLogger('job-queue');
  }

  setConcurrency(n: number): void {
    this.concurrency = Math.max(1, n);
    this.tick();
  }

  register<P, R>(kind: JobKind, handler: JobHandler<P, R>): void {
    this.handlers.set(kind, handler as JobHandler);
  }

  async enqueue<P>(kind: JobKind, payload: P, opts: EnqueueOptions = {}): Promise<JobRecord<P>> {
    const now = Date.now();
    const job: JobRecord<P> = {
      id: nanoid(),
      kind,
      status: 'queued',
      progress: 0,
      payload,
      retries: 0,
      maxRetries: opts.maxRetries ?? 2,
      createdAt: now,
      updatedAt: now,
    };
    this.jobs.set(job.id, job);
    this.pending.push(job.id);
    await this.persistor?.save(job);
    await this.emitEvent({ type: 'job.queued', jobId: job.id, ts: now });
    this.tick();
    return job;
  }

  get(jobId: string): JobRecord | undefined {
    return this.jobs.get(jobId);
  }

  list(): JobRecord[] {
    return [...this.jobs.values()];
  }

  async cancel(jobId: string): Promise<void> {
    const ac = this.aborts.get(jobId);
    if (ac) ac.abort();
    const job = this.jobs.get(jobId);
    if (
      job &&
      (job.status === 'queued' || job.status === 'running' || job.status === 'streaming')
    ) {
      this.updateStatus(job, 'cancelled');
      await this.emitEvent({ type: 'job.cancelled', jobId, ts: Date.now() });
    }
  }

  async retry(jobId: string): Promise<void> {
    const job = this.jobs.get(jobId);
    if (!job) return;
    if (job.status !== 'failed' && job.status !== 'cancelled') return;
    job.status = 'queued';
    job.retries = 0;
    job.error = undefined;
    job.updatedAt = Date.now();
    this.pending.push(jobId);
    await this.persistor?.save(job);
    this.tick();
  }

  async hydrate(): Promise<void> {
    if (!this.persistor) return;
    const records = await this.persistor.load();
    for (const r of records) {
      if (r.status === 'running' || r.status === 'streaming') {
        r.status = 'queued'; // recover interrupted jobs
        this.pending.push(r.id);
      }
      this.jobs.set(r.id, r);
    }
    this.tick();
  }

  private tick(): void {
    while (this.running < this.concurrency && this.pending.length > 0) {
      const id = this.pending.shift();
      if (!id) break;
      void this.execute(id);
    }
  }

  private async execute(jobId: string): Promise<void> {
    const job = this.jobs.get(jobId);
    if (!job) return;
    const handler = this.handlers.get(job.kind);
    if (!handler) {
      job.error = `No handler registered for ${job.kind}`;
      this.updateStatus(job, 'failed');
      await this.emitEvent({ type: 'job.failed', jobId, data: job.error, ts: Date.now() });
      return;
    }

    this.running++;
    const ac = new AbortController();
    this.aborts.set(jobId, ac);
    job.startedAt = Date.now();
    this.updateStatus(job, 'running');
    await this.emitEvent({ type: 'job.started', jobId, ts: job.startedAt });

    try {
      const result = await handler({
        job,
        signal: ac.signal,
        setProgress: (p) => {
          job.progress = Math.max(0, Math.min(1, p));
          job.updatedAt = Date.now();
          void this.emitEvent({
            type: 'job.progress',
            jobId,
            data: job.progress,
            ts: job.updatedAt,
          });
        },
        stream: (delta) => {
          if (job.status !== 'streaming') this.updateStatus(job, 'streaming');
          void this.emitEvent({ type: 'job.stream', jobId, data: delta, ts: Date.now() });
        },
        logger: this.logger.child({ jobId, kind: job.kind }),
      });
      job.result = result;
      job.finishedAt = Date.now();
      this.updateStatus(job, 'completed');
      await this.emitEvent({ type: 'job.completed', jobId, data: result, ts: job.finishedAt });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (ac.signal.aborted) {
        this.updateStatus(job, 'cancelled');
        await this.emitEvent({ type: 'job.cancelled', jobId, ts: Date.now() });
      } else if (job.retries < job.maxRetries) {
        job.retries++;
        job.status = 'retrying';
        job.updatedAt = Date.now();
        await this.persistor?.save(job);
        this.pending.push(jobId);
      } else {
        job.error = message;
        job.finishedAt = Date.now();
        this.updateStatus(job, 'failed');
        await this.emitEvent({ type: 'job.failed', jobId, data: message, ts: job.finishedAt });
      }
    } finally {
      this.aborts.delete(jobId);
      this.running--;
      this.tick();
    }
  }

  private updateStatus(job: JobRecord, status: JobStatus): void {
    job.status = status;
    job.updatedAt = Date.now();
    void this.persistor?.save(job);
  }

  private async emitEvent(event: JobEvent): Promise<void> {
    await this.bus.emit('job.event', event);
  }
}
