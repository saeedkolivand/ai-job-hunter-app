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
  runtimes.register(ai);
  runtimes.register(data);

  await runtimes.start();

  async function upsertPosting(runtime: typeof data, posting: Record<string, unknown>) {
    const db = runtime.db();
    const query = posting.externalId
      ? { source: posting.source, externalId: posting.externalId }
      : { url: posting.url };
    await new Promise<void>((resolve, reject) => {
      db.jobPostings.update(
        query,
        { $set: { ...posting, capturedAt: Date.now() } },
        { upsert: true },
        (err) => {
          if (err) reject(err);
          else resolve();
        }
      );
    });
  }

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
    const { board, query, location, pages } = ctx.job.payload as {
      board: string;
      query: string;
      location?: string;
      pages: number;
    };
    const scraper = data.scrapers.get(board);
    if (!scraper) throw new Error(`Unknown board: ${board}`);
    ctx.logger.info({ board, query, mode: scraper.mode }, 'scrape.board start');

    const credentialsAccessor = {
      get: async (id: string) => {
        const c = await credentials.getDecrypted(id);
        return c ? { username: c.username, password: c.password } : null;
      },
      storageStatePath: (id: string) => credentials.storageStatePath(id),
    };

    const { dateFilter, locale } = ctx.job.payload as { dateFilter?: string; locale?: string };

    data.liveJobs.clearAll();

    const results = await scraper.search(
      {
        query,
        ...(location ? { location } : {}),
        pages,
        ...(dateFilter ? { dateFilter } : {}),
        ...(locale ? { locale } : {}),
      },
      {
        signal: ctx.signal,
        onProgress: (p) => ctx.setProgress(p),
        onItem: (item) => {
          data.liveJobs.add(ctx.job.id, item);
          ctx.stream(item);
        },
        browser: electronBrowser as never,
        credentials: credentialsAccessor,
      }
    );

    return { board, count: results.length };
  });

  jobs.register('apply.job', async (ctx) => {
    const payload = ctx.job.payload as {
      board: string;
      url: string;
      coverLetter?: string;
      resumePath?: string;
      autoSubmit?: boolean;
    };
    const applier = data.appliers.get(payload.board);
    if (!applier) throw new Error(`No applier registered for ${payload.board}`);
    const cred = await credentials.getDecrypted(payload.board);
    ctx.logger.info(
      { board: payload.board, url: payload.url, autoSubmit: !!payload.autoSubmit },
      'apply.job start'
    );

    const result = await applier.apply(payload.url, {
      signal: ctx.signal,
      browser: electronBrowser as never,
      storageStatePath: credentials.storageStatePath(payload.board),
      credentials: cred ? { username: cred.username, password: cred.password } : null,
      ...(payload.coverLetter ? { coverLetter: payload.coverLetter } : {}),
      ...(payload.resumePath ? { resumePath: payload.resumePath } : {}),
      autoSubmit: !!payload.autoSubmit,
      onProgress: (p, stage) => {
        ctx.setProgress(p);
        ctx.stream({ kind: 'progress', stage, p });
      },
      onStep: (step) => ctx.stream({ kind: 'step', ...step }),
    });

    // Persist job when applied
    const credentialsAccessor = {
      get: async (id: string) => {
        const c = await credentials.getDecrypted(id);
        return c ? { username: c.username, password: c.password } : null;
      },
      storageStatePath: (id: string) => credentials.storageStatePath(id),
    };

    for (const s of data.scrapers.list()) {
      if (typeof s.fromUrl !== 'function') continue;
      const item = await s.fromUrl(payload.url, {
        signal: ctx.signal,
        browser: electronBrowser as never,
        credentials: credentialsAccessor,
      });
      if (item) {
        await upsertPosting(data, item);
        break;
      }
    }

    return result;
  });

  jobs.register('scrape.url', async (ctx) => {
    const { url } = ctx.job.payload as { url: string };
    const credentialsAccessor = {
      get: async (id: string) => {
        const c = await credentials.getDecrypted(id);
        return c ? { username: c.username, password: c.password } : null;
      },
      storageStatePath: (id: string) => credentials.storageStatePath(id),
    };
    for (const s of data.scrapers.list()) {
      if (typeof s.fromUrl !== 'function') continue;
      const item = await s.fromUrl(url, {
        signal: ctx.signal,
        browser: electronBrowser as never,
        credentials: credentialsAccessor,
      });
      if (item) {
        ctx.stream(item);
        return item;
      } // Stream without auto-persisting
    }
    throw new Error('No scraper accepted this URL');
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
  };
}
