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
import type { AiGenerateRequest } from '@ajh/shared';

import { type BoardSessionMap, createBoardSessions } from './board-sessions/index.js';
import { CredentialStore } from './credentials.js';
import { ElectronBrowserController } from './electron-browser-controller.js';
import { InProcessScraperRuntime } from './scraper-runtime.js';

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
  /** In-process scraper runtime — owns scrapeBoard / applyJob / scrapeUrl logic. */
  scraperRuntime: InProcessScraperRuntime;
  onShuttingDown?: () => Promise<void>;
}

export async function bootstrap(): Promise<AppCore> {
  const logger = createLogger('bootstrap');
  const bus = new EventBus();
  const jobs = new JobQueue(bus, { concurrency: 2 });
  const scheduler = new TaskScheduler();
  const runtimes = new RuntimeManager(bus);
  const state = new StateCoordinator(bus, { locale: 'en' });

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

  const ai = new AiRuntime(bus);
  const userDataDir = app.getPath('userData');
  const data = new DataRuntime(bus, { userDataDir });
  const credentials = new CredentialStore();

  // Persistent board sessions — one per board, using persist:<id> Electron
  // partitions. Chromium stores cookies on disk automatically; user logs in
  // once and the session survives app restarts.
  // Also refreshes state.json for any board with an existing valid session
  // so scrapers work immediately on startup without re-login.
  const boardSessions = await createBoardSessions(userDataDir);

  const electronBrowser = new ElectronBrowserController();
  const scraperRuntime = new InProcessScraperRuntime(data, credentials, electronBrowser);
  runtimes.register(ai);
  runtimes.register(data);

  await runtimes.start();

  // ── Register job handlers ─────────────────────────────────────────────

  // AI generation: streams deltas back over the EventBus → renderer subscribes
  // via window.api.ai.onStream (IPC_CHANNELS.ai.stream).
  jobs.register('ai.generate', async (ctx) => {
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
    const payload = ctx.job.payload as import('./scraper-runtime.js').ScrapeBoardPayload;
    return scraperRuntime.scrapeBoard(payload, {
      signal: ctx.signal,
      jobId: ctx.job.id,
      onProgress: (p) => ctx.setProgress(p),
      onItem: (item) => ctx.stream(item),
    });
  });

  jobs.register('apply.job', async (ctx) => {
    const payload = ctx.job.payload as import('./scraper-runtime.js').ApplyJobPayload;
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
      data.browser,
      autopilotStore,
      credentialsForAutopilot,
      ctx.job.id,
      ctx
    );
  });

  // ── Autopilot scheduler ───────────────────────────────────────────────────
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

  for (const [schedule, intervalMs] of Object.entries(SCHEDULE_INTERVALS)) {
    scheduler.every(`autopilot.${schedule}`, intervalMs, () => enqueueAutopilots(schedule));
  }

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
  };
}
