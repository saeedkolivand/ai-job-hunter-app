/**
 * createWebHttpClient — Web HTTP Adapter for AppClient.
 *
 * This is the third and final adapter in the three-path architecture:
 *
 *   React features
 *     → AppClient (typed commands + job events)
 *       → createDesktopIpcClient()   desktop Electron (today)
 *       → createTauriInvokeClient()  Tauri shell     (spike done)
 *       → createWebHttpClient()      Web / REST      (this file)
 *
 * The web adapter talks to a running runtime server (the same Node.js
 * processes that run as Tauri sidecars can also run as standalone HTTP
 * services). This lets the React app deploy on the web without any native
 * shell — the runtime server handles scraping, AI, and data behind an
 * authenticated REST + WebSocket API.
 *
 * ── Protocol ────────────────────────────────────────────────────────────────
 *  Commands:   POST /api/<namespace>/<method>
 *              Content-Type: application/json
 *              Body: request payload
 *              Response: application/json result
 *
 *  Streaming:  GET /api/events/<channel>  (Server-Sent Events)
 *              Returns text/event-stream, each event is JSON ScraperEvent
 *
 * ── Authentication ───────────────────────────────────────────────────────────
 *  The runtime server is expected to run on localhost and accept a
 *  per-launch random token passed in the Authorization header. Bind to
 *  127.0.0.1 only — never expose to a network interface without a proxy
 *  that enforces auth.
 *
 * ── Status ───────────────────────────────────────────────────────────────────
 *  This is a documented implementation skeleton. All commands are wired
 *  and typed — wire up the actual runtime server URL to get a working
 *  web client. Streaming event subscriptions use EventSource.
 *
 *  Replace the RUNTIME_BASE_URL below or pass it as a constructor argument.
 */
import type { AppClient } from './app-client';

export interface WebHttpClientOptions {
  /** Base URL of the runtime server (e.g. http://127.0.0.1:8742). */
  baseUrl: string;
  /**
   * Optional per-launch auth token placed in the Authorization header.
   * The runtime server generates this on startup and the shell passes it
   * to the renderer via a secure channel (IPC, environment variable, etc.).
   */
  token?: string;
}

type UnsubFn = () => void;

export function createWebHttpClient({ baseUrl, token }: WebHttpClientOptions): AppClient {
  const base = baseUrl.replace(/\/$/, '');
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  /** POST /api/<namespace>/<method> and return parsed JSON. */
  async function cmd<T = never>(namespace: string, method: string, payload?: unknown): Promise<T> {
    const res = await fetch(`${base}/api/${namespace}/${method}`, {
      method: 'POST',
      headers,
      body: payload !== undefined ? JSON.stringify(payload) : undefined,
    });
    if (!res.ok) {
      const msg = await res.text().catch(() => res.statusText);
      throw new Error(`[web-http] ${namespace}.${method} → ${res.status}: ${msg}`);
    }
    return res.json() as Promise<T>;
  }

  /**
   * Subscribe to a Server-Sent Events channel.
   * Returns a sync unsubscribe function (closes the EventSource).
   */
  function subscribe<T>(channel: string, handler: (event: T) => void): UnsubFn {
    const url = new URL(`${base}/api/events/${channel}`);
    if (token) url.searchParams.set('token', token);

    const es = new EventSource(url.toString());
    es.onmessage = (e) => {
      try {
        handler(JSON.parse(e.data as string) as T);
      } catch {
        // malformed event — ignore
      }
    };
    es.onerror = () => es.close();
    return () => es.close();
  }

  return {
    system: {
      health: () => cmd('system', 'health'),
      getVersion: () => cmd('system', 'getVersion'),
      getLocale: () => cmd('system', 'getLocale'),
      setLocale: (locale) => cmd('system', 'setLocale', { locale }),
      getPlatform: () => cmd('system', 'getPlatform'),
      openExternal: (url) => cmd('system', 'openExternal', { url }),
      setPerformanceMode: (mode) => cmd('system', 'setPerformanceMode', { mode }),
      getMetrics: () => cmd('system', 'getMetrics'),
    },

    jobs: {
      list: () => cmd('jobs', 'list'),
      get: (jobId) => cmd('jobs', 'get', { jobId }),
      cancel: (jobId) => cmd('jobs', 'cancel', { jobId }),
      retry: (jobId) => cmd('jobs', 'retry', { jobId }),
      onEvent: (handler) => subscribe('jobs.event', handler),
    },

    ai: {
      generate: (req) => cmd('ai', 'generate', req),
      listModels: () => cmd('ai', 'listModels'),
      pullModel: (model) => cmd('ai', 'pullModel', { model }),
      unloadModel: (model) => cmd('ai', 'unloadModel', { model }),
      embed: (req) => cmd('ai', 'embed', req),
      onStream: (handler) => subscribe('ai.stream', handler),
      setProviderKey: (req) => cmd('ai', 'setProviderKey', req),
      removeProviderKey: (req) => cmd('ai', 'removeProviderKey', req),
      hasProviderKey: (req) => cmd('ai', 'hasProviderKey', req),
      listProviderModels: (req) => cmd('ai', 'listProviderModels', req),
    },

    documents: {
      list: () => cmd('documents', 'list'),
      import: (req) => cmd('documents', 'import', req),
      remove: (id) => cmd('documents', 'remove', { id }),
      exportDocument: (req) => cmd('documents', 'exportDocument', req),
    },

    search: {
      hybrid: (req) => cmd('search', 'hybrid', req),
    },

    scrape: {
      board: (req) => cmd('scrape', 'board', req),
      url: (req) => cmd('scrape', 'url', req),
      persistJob: (req) => cmd('scrape', 'persistJob', req),
      listPostings: () => cmd('scrape', 'listPostings'),
      clearPostings: () => cmd('scrape', 'clearPostings'),
      listInteractions: (filter) => cmd('scrape', 'listInteractions', filter),
      exportData: () => cmd('scrape', 'exportData'),
      importData: () => cmd('scrape', 'importData'),
    },

    match: {
      resume: (req) => cmd('match', 'resume', req),
    },

    geocode: {
      suggest: (query: string) =>
        cmd('geocode', 'suggest', { query }) as Promise<Array<{ display: string }>>,
    },

    credentials: {
      available: () => cmd('credentials', 'available'),
      list: () => cmd('credentials', 'list'),
      set: (req) => cmd('credentials', 'set', req),
      remove: (boardId) => cmd('credentials', 'remove', { boardId }),
    },

    linkedin: {
      connect: () => cmd('linkedin', 'connect'),
      disconnect: () => cmd('linkedin', 'disconnect'),
      getStatus: () => cmd('linkedin', 'getStatus'),
    },

    boards: {
      connect: (boardId) => cmd('boards', 'connect', { boardId }),
      disconnect: (boardId) => cmd('boards', 'disconnect', { boardId }),
      getStatus: (boardId) => cmd('boards', 'getStatus', { boardId }),
    },

    privacy: {
      signOutAll: () => cmd('privacy', 'signOutAll'),
      clearInteractions: () => cmd('privacy', 'clearInteractions'),
    },

    apply: {
      start: (req) => cmd('apply', 'start', req),
      catalog: () => cmd('apply', 'catalog'),
    },

    updater: {
      check: () => cmd('updater', 'check'),
      download: () => cmd('updater', 'download'),
      install: () => cmd('updater', 'install'),
      onStatus: (handler) => subscribe('updater.status', handler),
    },

    shortcuts: {
      // Keyboard shortcuts on the web are handled entirely in the renderer —
      // no shell IPC needed. The handler is registered here as a no-op so the
      // feature code can still call onCommandPalette() without branching.
      onCommandPalette: (_handler) => () => {},
    },

    resume: {
      extractText: (req) => cmd('resume', 'extractText', req),
    },

    support: {
      exportDiagnostics: () => cmd('support', 'exportDiagnostics'),
      reloadAiRuntime: () => cmd('support', 'reloadAiRuntime'),
      unloadAllModels: () => cmd('support', 'unloadAllModels'),
      resetModelConfiguration: () => cmd('support', 'resetModelConfiguration'),
      rebuildVectorIndexes: () => cmd('support', 'rebuildVectorIndexes'),
      clearEmbeddingsCache: () => cmd('support', 'clearEmbeddingsCache'),
      resetVectorDatabase: () => cmd('support', 'resetVectorDatabase'),
      clearOcrCache: () => cmd('support', 'clearOcrCache'),
      reindexAllDocuments: () => cmd('support', 'reindexAllDocuments'),
      resetAllSessions: () => cmd('support', 'resetAllSessions'),
      clearScrapingQueue: () => cmd('support', 'clearScrapingQueue'),
      copyEnvironmentDetails: () => cmd('support', 'copyEnvironmentDetails'),
      copyAppVersion: () => cmd('support', 'copyAppVersion'),
      copySystemInfo: () => cmd('support', 'copySystemInfo'),
    },

    conversations: {
      getOrCreateConversation: () => cmd('conversations', 'getOrCreateConversation'),
      loadMessages: ({ conversationId }) =>
        cmd('conversations', 'loadMessages', { conversationId }),
      saveMessage: (req) => cmd('conversations', 'saveMessage', req),
      saveAllMessages: (opts) => cmd('conversations', 'saveAllMessages', opts),
    },

    autopilot: {
      list: () => cmd('autopilot', 'list'),
      get: ({ autopilotId }) => cmd('autopilot', 'get', { autopilotId }),
      create: (req) => cmd('autopilot', 'create', req),
      update: ({ autopilotId, ...data }) => cmd('autopilot', 'update', { autopilotId, ...data }),
      remove: ({ autopilotId }) => cmd('autopilot', 'remove', { autopilotId }),
      run: ({ autopilotId }) => cmd('autopilot', 'run', { autopilotId }),
      pause: ({ autopilotId }) => cmd('autopilot', 'pause', { autopilotId }),
      resume: ({ autopilotId }) => cmd('autopilot', 'resume', { autopilotId }),
    },

    dialog: {
      // Native file picker is a desktop capability. On web, use the browser's
      // <input type="file"> directly in the component — no IPC needed.
      openFiles: async () => [],
    },
  } satisfies AppClient;
}
