/**
 * Typed, capability-based IPC router.
 *
 * Every channel:
 *   - is namespaced
 *   - validates its payload with a Zod schema
 *   - returns a typed response or throws a serialisable error
 *
 * The renderer NEVER touches `ipcRenderer` directly; the preload bridge
 * exposes a narrow `window.api.*` object that mirrors `IpcContract`.
 */
import fs from 'node:fs/promises';

import { app, BrowserWindow, dialog, ipcMain, type IpcMainInvokeEvent, shell } from 'electron';
import { z, type ZodTypeAny } from 'zod';

import { createLogger } from '@ajh/core';
import { extractDocxFromBytes, extractPdfFromBytes } from '@ajh/data';
import {
  AiGenerateRequestSchema,
  ApplyStartSchema,
  AutopilotCreateSchema,
  AutopilotIdSchema,
  AutopilotUpdateSchema,
  CredentialBoardSchema,
  CredentialSetSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  HybridSearchRequestSchema,
  IPC_CHANNELS,
  JobIdSchema,
  LocaleSchema,
  MatchResumeRequestSchema,
  ResumeExtractTextSchema,
  ScrapeBoardRequestSchema,
  ScrapeUrlRequestSchema,
} from '@ajh/shared';

import type { AppCore } from '../bootstrap.js';
import { getStartupMs } from '../startup-metrics.js';

const logger = createLogger('ipc');

function handle<S extends ZodTypeAny | undefined, R>(
  channel: string,
  schema: S,
  fn: (input: S extends ZodTypeAny ? z.infer<S> : void, event: IpcMainInvokeEvent) => Promise<R>
): void {
  ipcMain.handle(channel, async (event, raw) => {
    try {
      const input = schema ? (schema as ZodTypeAny).parse(raw) : undefined;
      return await fn(input as never, event);
    } catch (err) {
      logger.error({ channel, err }, 'ipc handler failed');
      throw err instanceof Error ? err : new Error(String(err));
    }
  });
}

interface InteractionRecord {
  jobId: string;
  [key: string]: unknown;
}

export function registerIpc(core: AppCore): void {
  // ── system ──────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.system.health, undefined, async () => {
    const health = await core.runtimes.health();
    return {
      ai: (health.ai ?? { ready: false }) as { ready: boolean; model?: string },
      data: (health.data ?? { ready: false, sqlite: false, vector: false }) as {
        ready: boolean;
        sqlite: boolean;
        vector: boolean;
      },
      workers: { active: 0, idle: 0, max: 0 },
    };
  });

  handle(IPC_CHANNELS.system.getVersion, undefined, async () => app.getVersion());
  handle(IPC_CHANNELS.system.getLocale, undefined, async () => core.state.get('locale'));
  handle(IPC_CHANNELS.system.setLocale, LocaleSchema, async (loc) => {
    await core.state.set('locale', loc);
  });
  handle(IPC_CHANNELS.system.getPlatform, undefined, async () => process.platform);
  handle(IPC_CHANNELS.system.openExternal, z.string(), async (url) => {
    if (!/^https?:\/\//.test(url)) throw new Error('only http(s) allowed');
    await shell.openExternal(url);
  });

  handle(
    IPC_CHANNELS.system.setPerformanceMode,
    z.enum(['low-memory', 'balanced', 'performance']),
    async (mode) => {
      const concurrency = mode === 'low-memory' ? 1 : mode === 'performance' ? 4 : 2;
      const idleUnloadMs =
        mode === 'low-memory' ? 3 * 60_000 : mode === 'performance' ? 30 * 60_000 : 10 * 60_000;
      core.jobs.setConcurrency(concurrency);
      core.ai.setIdleUnloadMs(idleUnloadMs);
      logger.info({ mode, concurrency, idleUnloadMs }, 'performance mode applied');
    }
  );

  handle(IPC_CHANNELS.system.getMetrics, undefined, async () => {
    const raw = app.getAppMetrics();
    return {
      boot: core.bootMetrics,
      startupMs: getStartupMs(),
      jobQueue: core.jobs.metrics(),
      processes: raw.map((m) => ({
        pid: m.pid,
        type: m.type,
        cpuUsage: m.cpu,
        memory: {
          workingSetSize: m.memory.workingSetSize,
          peakWorkingSetSize: m.memory.peakWorkingSetSize,
        },
      })),
      snapshotAt: Date.now(),
    };
  });

  // ── jobs ────────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.jobs.list, undefined, async () => core.jobs.list());
  handle(IPC_CHANNELS.jobs.get, JobIdSchema, async ({ jobId }) => core.jobs.get(jobId) ?? null);
  handle(IPC_CHANNELS.jobs.cancel, JobIdSchema, async ({ jobId }) => {
    await core.jobs.cancel(jobId);
  });
  handle(IPC_CHANNELS.jobs.retry, JobIdSchema, async ({ jobId }) => {
    await core.jobs.retry(jobId);
  });

  // Forward job events to all renderer windows.
  // AI stream chunks ALSO go on a dedicated `ai:stream` channel so the
  // AI workspace can subscribe without filtering the generic event firehose.
  core.bus.on('job.event', (event) => {
    for (const w of BrowserWindow.getAllWindows()) {
      w.webContents.send(IPC_CHANNELS.jobs.event, event);
      if (event.type === 'job.stream') {
        const job = core.jobs.get(event.jobId);
        if (job?.kind === 'ai.generate') {
          const data = event.data as { delta: string; done: boolean } | undefined;
          w.webContents.send(IPC_CHANNELS.ai.stream, {
            jobId: event.jobId,
            delta: data?.delta ?? '',
            done: !!data?.done,
          });
        }
      }
    }
  });

  // ── ai ──────────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.ai.generate, AiGenerateRequestSchema, async (req) => {
    const job = await core.jobs.enqueue('ai.generate', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.ai.listModels, undefined, async () => {
    const health = await core.ai.health();
    logger.info({ health }, 'ai:listModels health check');
    const models = (health.models as string[]) ?? [];
    const result = models.map((name) => ({ name }));
    logger.info({ result, modelsCount: result.length }, 'ai:listModels returning');
    return result;
  });
  handle(IPC_CHANNELS.ai.pullModel, undefined, async (_: void, _e) => ({ jobId: '' }));
  handle(IPC_CHANNELS.ai.unloadModel, undefined, async () => {
    /* via runtime */
  });
  handle(IPC_CHANNELS.ai.embed, EmbedRequestSchema, async (req) => {
    try {
      const job = await core.jobs.enqueue('ai.embed', req);
      // Block until done — embeddings are short-lived. We poll the job record.
      // This keeps the renderer API simple (Promise<vector> rather than streamed).
      const result = await new Promise<{ vector: number[]; dim: number } | null>((resolve) => {
        const off = core.bus.on('job.event', (event) => {
          if (event.jobId !== job.id) return;
          if (event.type === 'job.completed') {
            off();
            resolve((event.data as { vector: number[]; dim: number }) ?? null);
          } else if (event.type === 'job.failed' || event.type === 'job.cancelled') {
            off();
            resolve(null);
          }
        });
      });
      return result;
    } catch {
      return null;
    }
  });

  // ── documents ───────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.documents.import, DocumentImportRequestSchema, async (req) => {
    const job = await core.jobs.enqueue('document.import', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.documents.list, undefined, async () => [] as never[]);
  handle(IPC_CHANNELS.documents.remove, undefined, async () => {
    /* via data runtime */
  });

  // ── search ──────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.search.hybrid, HybridSearchRequestSchema, async () => [] as never[]);

  // ── scrape ──────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.scrape.board, ScrapeBoardRequestSchema, async (req) => {
    const job = await core.jobs.enqueue('scrape.board', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.scrape.url, ScrapeUrlRequestSchema, async (req) => {
    const job = await core.jobs.enqueue('scrape.url', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.scrape.clearPostings, undefined, async () => {
    core.data.liveJobs.clearAll();
  });

  handle(
    IPC_CHANNELS.scrape.listInteractions,
    z.object({ interactionType: z.string().optional() }).optional(),
    async (filter) => {
      const db = core.data.db();
      const query = filter?.interactionType ? { interactionType: filter.interactionType } : {};
      return new Promise<InteractionRecord[]>((resolve, reject) => {
        db.jobInteractions
          .find(query)
          .sort({ timestamp: -1 })
          .exec((err, docs) => {
            if (err) reject(err);
            else resolve(docs as unknown as InteractionRecord[]);
          });
      });
    }
  );
  handle(IPC_CHANNELS.scrape.exportData, undefined, async () => {
    try {
      const db = core.data.db();

      // Collect all interactions
      const interactions = await new Promise<InteractionRecord[]>((resolve, reject) => {
        db.jobInteractions
          .find({})
          .sort({ timestamp: -1 })
          .exec((err, docs) => {
            if (err) reject(err);
            else resolve(docs as unknown as InteractionRecord[]);
          });
      });

      const bundle = {
        version: 1,
        exportedAt: Date.now(),
        appVersion: app.getVersion(),
        interactions,
      };

      const defaultName = `ajh-export-${new Date().toISOString().slice(0, 10)}.json`;
      const { filePath, canceled } = await dialog.showSaveDialog({
        title: 'Export App Data',
        defaultPath: defaultName,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });

      if (canceled || !filePath) return { success: false };

      await fs.writeFile(filePath, JSON.stringify(bundle, null, 2), 'utf-8');
      return { success: true, filePath };
    } catch (err) {
      return { success: false, error: err instanceof Error ? err.message : String(err) };
    }
  });

  handle(IPC_CHANNELS.scrape.importData, undefined, async () => {
    try {
      const { filePaths, canceled } = await dialog.showOpenDialog({
        title: 'Import App Data',
        filters: [{ name: 'JSON', extensions: ['json'] }],
        properties: ['openFile'],
      });

      if (canceled || !filePaths[0]) return { success: false, imported: 0 };

      const raw = await fs.readFile(filePaths[0], 'utf-8');
      const bundle = JSON.parse(raw);

      if (!bundle.version || !Array.isArray(bundle.interactions)) {
        return { success: false, imported: 0, error: 'Invalid export file format.' };
      }

      const db = core.data.db();
      let imported = 0;

      for (const record of bundle.interactions) {
        if (!record.jobId || !record.interactionType) continue;
        await new Promise<void>((resolve, reject) => {
          db.jobInteractions.update(
            { jobId: record.jobId, interactionType: record.interactionType },
            {
              $set: {
                jobId: record.jobId,
                interactionType: record.interactionType,
                timestamp: record.timestamp ?? Date.now(),
                title: record.title ?? '',
                company: record.company ?? '',
                url: record.url ?? '',
                source: record.source ?? '',
                location: record.location ?? '',
              },
            },
            { upsert: true },
            (err) => {
              if (err) reject(err);
              else resolve();
            }
          );
        });
        imported++;
      }

      return { success: true, imported };
    } catch (err) {
      return {
        success: false,
        imported: 0,
        error: err instanceof Error ? err.message : String(err),
      };
    }
  });

  handle(IPC_CHANNELS.scrape.persistJob, undefined, async (req) => {
    const job = await core.jobs.enqueue('persist.job', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.scrape.listPostings, undefined, async () => {
    try {
      const jobs = core.data.liveJobs.getAll();
      if (jobs.length === 0) return [];

      const db = core.data.db();
      const jobIds = jobs.map((j) => j.id);

      const interactions = await new Promise<InteractionRecord[]>((resolve, reject) => {
        db.jobInteractions.find({ jobId: { $in: jobIds } }).exec((err, docs) => {
          if (err) reject(err);
          else resolve(docs as unknown as InteractionRecord[]);
        });
      });

      const interactionMap = new Map<string, InteractionRecord[]>();
      for (const interaction of interactions) {
        if (!interactionMap.has(interaction.jobId)) interactionMap.set(interaction.jobId, []);
        interactionMap.get(interaction.jobId)?.push(interaction);
      }

      return jobs.map((job) => ({ ...job, interactions: interactionMap.get(job.id) || [] }));
    } catch (error) {
      const logger = createLogger('ipc.scrape.listPostings');
      logger.error({ error }, 'Error returning job postings');
      return [];
    }
  });

  // ── match ───────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.match.resume, MatchResumeRequestSchema, async () => ({
    resumeId: '',
    jobId: '',
    ats: 0,
    semantic: 0,
    combined: 0,
    gaps: [],
    recommendations: [],
    explanation: '',
  }));

  // ── credentials ─────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.credentials.available, undefined, async () =>
    core.credentials.isEncryptionAvailable()
  );
  handle(IPC_CHANNELS.credentials.list, undefined, async () => core.credentials.list());
  handle(IPC_CHANNELS.credentials.set, CredentialSetSchema, async (req) => {
    await core.credentials.set(req.boardId, req.username, req.password);
  });
  handle(IPC_CHANNELS.credentials.remove, CredentialBoardSchema, async (req) => {
    await core.credentials.remove(req.boardId);
  });

  // ── boards (generic per-board connect/disconnect/status) ──────────────────
  handle(IPC_CHANNELS.boards.connect, CredentialBoardSchema, async ({ boardId }) => {
    const sess = core.boardSessions.get(boardId);
    if (!sess) throw new Error(`Unknown board: ${boardId}`);
    const status = await sess.connect();
    return { connected: status.connected, accountEmail: status.accountEmail };
  });
  handle(IPC_CHANNELS.boards.disconnect, CredentialBoardSchema, async ({ boardId }) => {
    const sess = core.boardSessions.get(boardId);
    if (!sess) return;
    await sess.disconnect();
  });
  handle(IPC_CHANNELS.boards.getStatus, CredentialBoardSchema, async ({ boardId }) => {
    const sess = core.boardSessions.get(boardId);
    if (!sess) return { connected: false };
    const status = await sess.getStatus();
    return { connected: status.connected, accountEmail: status.accountEmail };
  });

  // ── linkedin (legacy endpoints — delegate to boards) ───────────────────────
  handle(IPC_CHANNELS.linkedin.connect, undefined, async () => {
    const sess = core.boardSessions.get('linkedin');
    if (!sess) throw new Error('LinkedIn session manager not found');
    const status = await sess.connect();
    return { connected: status.connected, accountEmail: status.accountEmail };
  });
  handle(IPC_CHANNELS.linkedin.disconnect, undefined, async () => {
    await core.boardSessions.get('linkedin')?.disconnect();
  });
  handle(IPC_CHANNELS.linkedin.getStatus, undefined, async () => {
    const sess = core.boardSessions.get('linkedin');
    if (!sess) return { connected: false };
    const status = await sess.getStatus();
    return { connected: status.connected, accountEmail: status.accountEmail };
  });

  // ── privacy ───────────────────────────────────────────────────────────────
  /**
   * Reset every chromium profile so the next "Connect" launches a fresh browser.
   * Closes any running persistent contexts first to release Windows file locks,
   * then deletes the on-disk profile dirs.
   */
  async function resetChromiumProfiles(): Promise<void> {
    const fs = await import('node:fs/promises');
    const path = await import('node:path');
    const userDataDir = app.getPath('userData');
    const browserStateDir = path.join(userDataDir, 'browser-state');
    const linkedinSessionDir = path.join(userDataDir, 'linkedin-session');
    const boardSessionsDir = path.join(userDataDir, 'board-sessions');

    // Electron partition directories are already cleared by sess.disconnect() above.
    // No Playwright contexts to release — ElectronBrowserController manages sessions
    // via Electron's built-in session API, not on-disk context directories.

    try {
      const boards = await fs.readdir(browserStateDir);
      for (const board of boards) {
        await fs.rm(path.join(browserStateDir, board), { recursive: true, force: true });
      }
    } catch {
      /* ignore */
    }
    try {
      await fs.rm(linkedinSessionDir, { recursive: true, force: true });
    } catch {
      /* ignore */
    }
    try {
      await fs.rm(boardSessionsDir, { recursive: true, force: true });
    } catch {
      /* ignore */
    }
  }

  handle(IPC_CHANNELS.privacy.signOutAll, undefined, async () => {
    // Disconnect all persistent board sessions (clears Chromium data + state.json).
    for (const sess of core.boardSessions.values()) {
      await sess.disconnect().catch(() => {});
    }
    await resetChromiumProfiles();
  });

  handle(IPC_CHANNELS.privacy.clearInteractions, undefined, async () => {
    const db = core.data.db();
    await new Promise<void>((resolve, reject) => {
      db.jobInteractions.remove({}, { multi: true }, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  });

  // ── apply ───────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.apply.start, ApplyStartSchema, async (req) => {
    const job = await core.jobs.enqueue('apply.job', req);
    return { jobId: job.id };
  });
  handle(IPC_CHANNELS.apply.catalog, undefined, async () => core.data.appliers.catalog());

  // ── resume ──────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.resume.extractText, ResumeExtractTextSchema, async ({ name, bytes }) => {
    const ext = name.toLowerCase().split('.').pop() ?? '';
    if (ext === 'pdf') {
      const { text } = await extractPdfFromBytes(bytes);
      return { text };
    }
    if (ext === 'docx') {
      return await extractDocxFromBytes(bytes);
    }
    if (ext === 'txt' || ext === 'md' || ext === 'markdown') {
      return { text: new TextDecoder('utf-8').decode(bytes).trim() };
    }
    throw new Error(`unsupported file type: .${ext}`);
  });

  // ── support ─────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.support.exportDiagnostics, undefined, async () => {
    // TODO: Implement diagnostics bundle export
    return { success: false };
  });
  handle(IPC_CHANNELS.support.reloadAiRuntime, undefined, async () => {
    try {
      // TODO: Implement AI runtime restart when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.unloadAllModels, undefined, async () => {
    try {
      // TODO: Implement unload all models when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.resetModelConfiguration, undefined, async () => {
    // TODO: Implement model configuration reset
    return { success: false };
  });
  handle(IPC_CHANNELS.support.rebuildVectorIndexes, undefined, async () => {
    try {
      // TODO: Implement vector index rebuild when job kind is available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.clearEmbeddingsCache, undefined, async () => {
    try {
      // TODO: Implement embeddings cache clear when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.resetVectorDatabase, undefined, async () => {
    // TODO: Implement vector database reset (destructive)
    return { success: false };
  });
  handle(IPC_CHANNELS.support.clearOcrCache, undefined, async () => {
    try {
      // TODO: Implement OCR cache clear when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.reindexAllDocuments, undefined, async () => {
    try {
      // TODO: Implement document reindex when job kind is available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.resetAllSessions, undefined, async () => {
    try {
      // TODO: Implement clear all sessions when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });
  handle(IPC_CHANNELS.support.clearScrapingQueue, undefined, async () => {
    try {
      // TODO: Implement clear scraping queue when available
      return { success: false };
    } catch {
      return { success: false };
    }
  });

  // ── conversations ───────────────────────────────────────────────────────
  const SaveMessageSchema = z.object({
    conversationId: z.string().min(1),
    role: z.enum(['user', 'assistant', 'system']),
    content: z.string().min(1),
  });

  // ── autopilot ─────────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.autopilot.list, undefined, async () => {
    return core.autopilotStore.list();
  });

  handle(IPC_CHANNELS.autopilot.get, AutopilotIdSchema, async ({ autopilotId }) => {
    return core.autopilotStore.get(autopilotId);
  });

  handle(IPC_CHANNELS.autopilot.create, AutopilotCreateSchema, async (req) => {
    const ap = await core.autopilotStore.create(req);
    void core.refreshScheduler();
    return ap;
  });

  handle(
    IPC_CHANNELS.autopilot.update,
    AutopilotUpdateSchema.extend({ autopilotId: z.string().min(1) }),
    async ({ autopilotId, ...patch }) => {
      const ap = await core.autopilotStore.update(autopilotId, patch);
      void core.refreshScheduler();
      return ap;
    }
  );

  handle(IPC_CHANNELS.autopilot.remove, AutopilotIdSchema, async ({ autopilotId }) => {
    await core.autopilotStore.remove(autopilotId);
    void core.refreshScheduler();
  });

  handle(IPC_CHANNELS.autopilot.run, AutopilotIdSchema, async ({ autopilotId }) => {
    const ap = await core.autopilotStore.get(autopilotId);
    if (!ap) throw new Error(`Autopilot not found: ${autopilotId}`);
    const job = await core.jobs.enqueue('autopilot.run', { autopilotId });
    return { jobId: job.id };
  });

  handle(IPC_CHANNELS.autopilot.pause, AutopilotIdSchema, async ({ autopilotId }) => {
    await core.autopilotStore.setStatus(autopilotId, 'paused');
  });

  handle(IPC_CHANNELS.autopilot.resume, AutopilotIdSchema, async ({ autopilotId }) => {
    await core.autopilotStore.setStatus(autopilotId, 'active');
  });

  // ── conversations ────────────────────────────────────────────────────────
  handle(IPC_CHANNELS.conversations.getOrCreateConversation, undefined, async () => {
    try {
      return { id: 'default', title: 'AI Chat' };
    } catch (error) {
      console.error('Failed to get or create conversation:', error);
      return { id: 'default', title: 'AI Chat' };
    }
  });

  handle(
    IPC_CHANNELS.conversations.loadMessages,
    z.object({ conversationId: z.string().min(1) }),
    async (_) => {
      return [];
    }
  );

  handle(IPC_CHANNELS.conversations.saveMessage, SaveMessageSchema, async (_) => {
    // No-op - no persistence
    return { success: true };
  });
}
