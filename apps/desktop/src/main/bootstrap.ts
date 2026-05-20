/**
 * Bootstraps the Application Core:
 *   EventBus → JobQueue → TaskScheduler → RuntimeManager → StateCoordinator
 * and registers the AI Runtime + Data Runtime.
 *
 * Returns a `core` handle used by the IPC router and shutdown logic.
 */
import { app } from 'electron';

import { AiRuntime, generateStream } from '@ajh/ai';
import {
  createLogger,
  EventBus,
  JobQueue,
  RuntimeManager,
  StateCoordinator,
  TaskScheduler,
} from '@ajh/core';
import { AutopilotStore, DataRuntime, runAutopilot } from '@ajh/data';
import type { AiGenerateRequest, BootMetrics } from '@ajh/shared';

import { type BoardSessionMap, createBoardSessions } from './board-sessions/index.js';
import { CredentialStore } from './credentials.js';
import { ElectronBrowserController } from './electron-browser-controller.js';
import {
  type ApplyJobPayload,
  InProcessScraperRuntime,
  type ScrapeBoardPayload,
  type ScraperRuntimeClient,
} from './scraper-runtime.js';

export interface AppCore {
  bus: EventBus;
  jobs: JobQueue;
  scheduler: TaskScheduler;
  runtimes: RuntimeManager;
  state: StateCoordinator<{ locale: string; activeModel?: string }>;
  ai: AiRuntime;
  data: DataRuntime;
  credentials: CredentialStore;
  autopilotStore: AutopilotStore;
  /** Persistent Chromium session managers — one per board, survive restarts. */
  boardSessions: BoardSessionMap;
  /** Active scraper runtime — in-process today, utility-process or sidecar later. */
  scraperRuntime: ScraperRuntimeClient;
  /** Electron-native browser controller — single source of truth for all browser automation. */
  electronBrowser: ElectronBrowserController;
  /** Re-evaluate whether the autopilot scheduler should run. Call after create/update/remove. */
  refreshScheduler: () => Promise<void>;
  onShuttingDown?: () => Promise<void>;
  /** Timing breakdown of the bootstrap() call — set once, never changes. */
  bootMetrics: BootMetrics;
}

export async function bootstrap(): Promise<AppCore> {
  const logger = createLogger('bootstrap');
  const bootStart = performance.now();
  const bootStartedAt = Date.now();

  // Platform detection log
  logger.info(
    {
      platform: process.platform,
      arch: process.arch,
      nodeVersion: process.version,
      electronVersion: process.versions.electron,
    },
    'platform detected'
  );

  const t0 = performance.now();
  const bus = new EventBus();
  const jobs = new JobQueue(bus, { concurrency: 2 });
  const scheduler = new TaskScheduler();
  const runtimes = new RuntimeManager(bus);
  const state = new StateCoordinator(bus, { locale: 'en' });
  const phCoreInit = performance.now() - t0;

  const ai = new AiRuntime(bus);
  const userDataDir = app.getPath('userData');
  const data = new DataRuntime(bus, { userDataDir });
  const credentials = new CredentialStore();

  // Persistent board sessions — one per board, using persist:<id> Electron
  // partitions. Chromium stores cookies on disk automatically; user logs in
  // once and the session survives app restarts.
  // Also refreshes state.json for any board with an existing valid session
  // so scrapers work immediately on startup without re-login.
  const t1 = performance.now();
  const boardSessions = await createBoardSessions(userDataDir);
  const phBoardSessions = performance.now() - t1;

  const electronBrowser = new ElectronBrowserController();

  // AJH_SCRAPER_MODE=in-process forces the in-process fallback (rollback switch).
  // Default is in-process today; swap for UtilityProcessScraperRuntime in Phase 6.
  const scraperMode = process.env.AJH_SCRAPER_MODE ?? 'in-process';
  const scraperRuntime: ScraperRuntimeClient = new InProcessScraperRuntime(
    data,
    credentials,
    electronBrowser
  );
  logger.info({ scraperMode }, 'scraper runtime selected');
  runtimes.register(ai);
  runtimes.register(data);

  // Start data runtime eagerly — SQLite is needed immediately by autopilotStore
  // and the scheduler. AI runtime starts lazily on first ai.generate/ai.embed job.
  const t2 = performance.now();
  await runtimes.start('data');
  const phDataRuntime = performance.now() - t2;

  // ── Register job handlers ─────────────────────────────────────────────
  const t3 = performance.now();

  // AI generation: streams deltas back over the EventBus → renderer subscribes
  // via window.api.ai.onStream (IPC_CHANNELS.ai.stream).
  jobs.register('ai.generate', async (ctx) => {
    await runtimes.start('ai');
    const req = ctx.job.payload as AiGenerateRequest;
    const client = ai.getClient();
    let full = '';
    let lastTick = 0;
    try {
      for await (const { delta, done } of generateStream(client, req, ctx.signal)) {
        if (ctx.signal.aborted) break;
        if (delta) full += delta;
        // Always emit when there's content or when signalling completion,
        // so the renderer's done listener reliably fires even on an empty final chunk.
        if (delta || done) ctx.stream({ delta, done });
        if (done) break;
        const now = Date.now();
        if (now - lastTick > 100) {
          ctx.setProgress(Math.min(0.95, full.length / 4000));
          lastTick = now;
        }
      }
      ai.markUsed(req.model, 'reasoning');
      return { text: full };
    } catch (err) {
      ctx.logger.error({ err }, 'ai.generate failed');
      throw err;
    }
  });

  jobs.register('ai.embed', async (ctx) => {
    await runtimes.start('ai');
    const { text, model } = ctx.job.payload as { text: string; model?: string };
    const client = ai.getClient();
    const m = model ?? 'nomic-embed-text';
    const vectors = await client.embed(m, text);
    const v = vectors[0];
    if (!v) throw new Error('no embedding produced');
    ai.markUsed(m, 'embedding', v.length);
    return { vector: v, dim: v.length };
  });

  jobs.register('scrape.board', async (ctx) => {
    const payload = ctx.job.payload as ScrapeBoardPayload;
    return scraperRuntime.scrapeBoard(payload, {
      signal: ctx.signal,
      jobId: ctx.job.id,
      onProgress: (p) => ctx.setProgress(p),
      onItem: (item) => ctx.stream(item),
    });
  });

  jobs.register('apply.job', async (ctx) => {
    const payload = ctx.job.payload as ApplyJobPayload;
    return scraperRuntime.applyJob(payload, {
      signal: ctx.signal,
      onProgress: (p, stage) => {
        ctx.setProgress(p);
        ctx.stream({ kind: 'progress', stage, p });
      },
      onStep: (step) => ctx.stream({ kind: 'step', ...step }),
    });
  });

  jobs.register('scrape.url', async (ctx) => {
    const payload = ctx.job.payload as { url: string };
    return scraperRuntime.scrapeUrl(payload, {
      signal: ctx.signal,
      onItem: (item) => ctx.stream(item),
    });
  });

  // Track user interaction with a job (viewed, applied, bookmarked)
  jobs.register('persist.job', async (ctx) => {
    const { job, interactionType } = ctx.job.payload as {
      job: {
        id: string;
        title?: string;
        company?: string;
        url?: string;
        source?: string;
        location?: string;
      };
      interactionType: string;
    };
    const db = data.db();

    await new Promise<void>((resolve, reject) => {
      db.jobInteractions.update(
        { jobId: job.id, interactionType },
        {
          $set: {
            jobId: job.id,
            interactionType,
            timestamp: Date.now(),
            title: job.title ?? '',
            company: job.company ?? '',
            url: job.url ?? '',
            source: job.source ?? '',
            location: job.location ?? '',
          },
        },
        { upsert: true },
        (err) => {
          if (err) reject(err);
          else resolve();
        }
      );
    });

    return { success: true, jobId: job.id };
  });

  const phJobHandlers = performance.now() - t3;

  logger.info({ scrapers: data.scrapers.catalog().length }, 'core bootstrapped');

  // ── Autopilot store ───────────────────────────────────────────────────────
  const autopilotStore = new AutopilotStore(data.db().autopilots);

  // ── autopilot.run job handler ─────────────────────────────────────────────
  const credentialsForAutopilot = {
    get: async (boardId: string) => {
      const c = await credentials.getDecrypted(boardId);
      return c ? { username: c.username, password: c.password } : null;
    },
    storageStatePath: (boardId: string) => credentials.storageStatePath(boardId),
  };

  jobs.register('autopilot.run', async (ctx) => {
    const { autopilotId } = ctx.job.payload as { autopilotId: string };
    const ap = await autopilotStore.get(autopilotId);
    if (!ap) throw new Error(`Autopilot not found: ${autopilotId}`);
    if (ap.status === 'paused') {
      ctx.logger.info({ autopilotId }, 'autopilot is paused, skipping run');
      return { skipped: true };
    }
    return runAutopilot(
      ap,
      data.scrapers,
      data.appliers,
      electronBrowser as never,
      autopilotStore,
      credentialsForAutopilot,
      ctx.job.id,
      ctx
    );
  });

  // ── Autopilot scheduler ───────────────────────────────────────────────────
  // Only start schedule intervals when at least one active autopilot exists.
  // Call refreshScheduler() after any create/update/remove to keep in sync.
  const SCHEDULE_INTERVALS: Record<string, number> = {
    hourly: 60 * 60 * 1000,
    twice_daily: 12 * 60 * 60 * 1000,
    daily: 24 * 60 * 60 * 1000,
  };

  const enqueueAutopilots = async (schedule: string) => {
    try {
      const pilots = await autopilotStore.listBySchedule(schedule);
      for (const ap of pilots) {
        logger.info({ autopilotId: ap._id, schedule }, 'scheduler triggering autopilot');
        await jobs.enqueue('autopilot.run', { autopilotId: ap._id });
      }
    } catch (err) {
      logger.error({ err, schedule }, 'autopilot scheduler tick failed');
    }
  };

  const refreshScheduler = async () => {
    try {
      const all = await autopilotStore.list();
      const hasActive = all.some((ap) => ap.status !== 'paused');
      const running = scheduler.has('autopilot.hourly');
      if (hasActive && !running) {
        for (const [schedule, intervalMs] of Object.entries(SCHEDULE_INTERVALS)) {
          scheduler.every(`autopilot.${schedule}`, intervalMs, () => enqueueAutopilots(schedule));
        }
        logger.info({ count: all.length }, 'autopilot scheduler started');
      } else if (!hasActive && running) {
        for (const schedule of Object.keys(SCHEDULE_INTERVALS)) {
          scheduler.cancel(`autopilot.${schedule}`);
        }
        logger.info('autopilot scheduler stopped — no active autopilots');
      }
    } catch (err) {
      logger.warn({ err }, 'refreshScheduler failed');
    }
  };

  // Start only if active autopilots already exist.
  const t4 = performance.now();
  await refreshScheduler();
  const phScheduler = performance.now() - t4;

  const bootTotalMs = performance.now() - bootStart;
  const bootMetrics: BootMetrics = {
    startedAt: bootStartedAt,
    phases: {
      coreInit: phCoreInit,
      boardSessions: phBoardSessions,
      dataRuntime: phDataRuntime,
      jobHandlers: phJobHandlers,
      scheduler: phScheduler,
    },
    totalMs: bootTotalMs,
  };

  logger.info(
    {
      totalMs: Math.round(bootTotalMs),
      phases: {
        coreInit: Math.round(phCoreInit),
        boardSessions: Math.round(phBoardSessions),
        dataRuntime: Math.round(phDataRuntime),
        jobHandlers: Math.round(phJobHandlers),
        scheduler: Math.round(phScheduler),
      },
    },
    'bootstrap complete'
  );

  // Top-level shutdown hook (called from main on before-quit)
  (global as { __ajh_shutdown?: () => Promise<void> }).__ajh_shutdown = async () => {
    scheduler.cancelAll();
    await runtimes.stop();
  };

  return {
    bus,
    jobs,
    scheduler,
    runtimes,
    state,
    ai,
    data,
    credentials,
    autopilotStore,
    boardSessions,
    scraperRuntime,
    electronBrowser,
    refreshScheduler,
    bootMetrics,
  };
}
