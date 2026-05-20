/**
 * Preload bridge — exposes a narrow `window.api` typed against `IpcContract`.
 *
 * The renderer NEVER sees ipcRenderer or any Electron primitive. Every method
 * is a thin proxy that resolves with the IPC result. Streaming/event channels
 * return an unsubscribe function.
 */
import { contextBridge, ipcRenderer } from 'electron';
import { IPC_CHANNELS } from '@ajh/shared';

const invoke = (ch: string, payload?: unknown) => ipcRenderer.invoke(ch, payload);

const api = {
  system: {
    health: () => invoke(IPC_CHANNELS.system.health),
    getVersion: () => invoke(IPC_CHANNELS.system.getVersion),
    getLocale: () => invoke(IPC_CHANNELS.system.getLocale),
    setLocale: (locale: string) => invoke(IPC_CHANNELS.system.setLocale, locale),
    getPlatform: () => invoke(IPC_CHANNELS.system.getPlatform),
    openExternal: (url: string) => invoke(IPC_CHANNELS.system.openExternal, url),
  },
  jobs: {
    list: () => invoke(IPC_CHANNELS.jobs.list),
    get: (jobId: string) => invoke(IPC_CHANNELS.jobs.get, { jobId }),
    cancel: (jobId: string) => invoke(IPC_CHANNELS.jobs.cancel, { jobId }),
    retry: (jobId: string) => invoke(IPC_CHANNELS.jobs.retry, { jobId }),
    onEvent: (handler: (event: unknown) => void) => {
      const listener = (_: unknown, event: unknown) => handler(event);
      ipcRenderer.on(IPC_CHANNELS.jobs.event, listener);
      return () => ipcRenderer.off(IPC_CHANNELS.jobs.event, listener);
    },
  },
  ai: {
    generate: (req: unknown) => invoke(IPC_CHANNELS.ai.generate, req),
    listModels: () => invoke(IPC_CHANNELS.ai.listModels),
    pullModel: (model: string) => invoke(IPC_CHANNELS.ai.pullModel, model),
    unloadModel: (model: string) => invoke(IPC_CHANNELS.ai.unloadModel, model),
    embed: (req: { text: string; model?: string }) => invoke(IPC_CHANNELS.ai.embed, req),
    onStream: (handler: (chunk: unknown) => void) => {
      const listener = (_: unknown, chunk: unknown) => handler(chunk);
      ipcRenderer.on(IPC_CHANNELS.ai.stream, listener);
      return () => ipcRenderer.off(IPC_CHANNELS.ai.stream, listener);
    },
  },
  documents: {
    list: () => invoke(IPC_CHANNELS.documents.list),
    import: (req: unknown) => invoke(IPC_CHANNELS.documents.import, req),
    remove: (id: string) => invoke(IPC_CHANNELS.documents.remove, id),
  },
  search: {
    hybrid: (req: unknown) => invoke(IPC_CHANNELS.search.hybrid, req),
  },
  scrape: {
    board: (req: unknown) => invoke(IPC_CHANNELS.scrape.board, req),
    url: (req: unknown) => invoke(IPC_CHANNELS.scrape.url, req),
    persistJob: (req: unknown) => invoke(IPC_CHANNELS.scrape.persistJob, req),
    listPostings: () => invoke(IPC_CHANNELS.scrape.listPostings),
    clearPostings: () => invoke(IPC_CHANNELS.scrape.clearPostings),
    listInteractions: (filter?: { interactionType?: string }) =>
      invoke(IPC_CHANNELS.scrape.listInteractions, filter),
    exportData: () => invoke(IPC_CHANNELS.scrape.exportData),
    importData: () => invoke(IPC_CHANNELS.scrape.importData),
  },
  match: {
    resume: (req: unknown) => invoke(IPC_CHANNELS.match.resume, req),
  },
  credentials: {
    available: () => invoke(IPC_CHANNELS.credentials.available),
    list: () => invoke(IPC_CHANNELS.credentials.list),
    set: (req: { boardId: string; username: string; password: string }) =>
      invoke(IPC_CHANNELS.credentials.set, req),
    remove: (boardId: string) => invoke(IPC_CHANNELS.credentials.remove, { boardId }),
  },
  linkedin: {
    connect: () => invoke(IPC_CHANNELS.linkedin.connect),
    disconnect: () => invoke(IPC_CHANNELS.linkedin.disconnect),
    getStatus: () => invoke(IPC_CHANNELS.linkedin.getStatus),
  },
  boards: {
    connect: (boardId: string) => invoke(IPC_CHANNELS.boards.connect, { boardId }),
    disconnect: (boardId: string) => invoke(IPC_CHANNELS.boards.disconnect, { boardId }),
    getStatus: (boardId: string) => invoke(IPC_CHANNELS.boards.getStatus, { boardId }),
  },
  privacy: {
    signOutAll: () => invoke(IPC_CHANNELS.privacy.signOutAll),
    clearInteractions: () => invoke(IPC_CHANNELS.privacy.clearInteractions),
  },
  apply: {
    start: (req: {
      board: string;
      url: string;
      coverLetter?: string;
      resumePath?: string;
      autoSubmit?: boolean;
    }) => invoke(IPC_CHANNELS.apply.start, req),
    catalog: () => invoke(IPC_CHANNELS.apply.catalog),
  },
  updater: {
    check: () => invoke(IPC_CHANNELS.updater.check),
    download: () => invoke(IPC_CHANNELS.updater.download),
    install: () => invoke(IPC_CHANNELS.updater.install),
    onStatus: (handler: (status: unknown) => void) => {
      const listener = (_: unknown, status: unknown) => handler(status);
      ipcRenderer.on(IPC_CHANNELS.updater.onStatus, listener);
      return () => ipcRenderer.off(IPC_CHANNELS.updater.onStatus, listener);
    },
  },
  shortcuts: {
    onCommandPalette: (handler: () => void) => {
      const listener = () => handler();
      ipcRenderer.on('shortcut:command-palette', listener);
      return () => ipcRenderer.off('shortcut:command-palette', listener);
    },
  },
  resume: {
    extractText: (req: { name: string; bytes: Uint8Array }) =>
      invoke(IPC_CHANNELS.resume.extractText, req),
  },
  support: {
    exportDiagnostics: () => invoke(IPC_CHANNELS.support.exportDiagnostics),
    reloadAiRuntime: () => invoke(IPC_CHANNELS.support.reloadAiRuntime),
    unloadAllModels: () => invoke(IPC_CHANNELS.support.unloadAllModels),
    resetModelConfiguration: () => invoke(IPC_CHANNELS.support.resetModelConfiguration),
    rebuildVectorIndexes: () => invoke(IPC_CHANNELS.support.rebuildVectorIndexes),
    clearEmbeddingsCache: () => invoke(IPC_CHANNELS.support.clearEmbeddingsCache),
    resetVectorDatabase: () => invoke(IPC_CHANNELS.support.resetVectorDatabase),
    clearOcrCache: () => invoke(IPC_CHANNELS.support.clearOcrCache),
    reindexAllDocuments: () => invoke(IPC_CHANNELS.support.reindexAllDocuments),
    resetAllSessions: () => invoke(IPC_CHANNELS.support.resetAllSessions),
    clearScrapingQueue: () => invoke(IPC_CHANNELS.support.clearScrapingQueue),
    copyEnvironmentDetails: () => invoke(IPC_CHANNELS.support.copyEnvironmentDetails),
    copyAppVersion: () => invoke(IPC_CHANNELS.support.copyAppVersion),
    copySystemInfo: () => invoke(IPC_CHANNELS.support.copySystemInfo),
  },
  conversations: {
    getOrCreateConversation: () => invoke(IPC_CHANNELS.conversations.getOrCreateConversation),
    loadMessages: (conversationId: string) =>
      invoke(IPC_CHANNELS.conversations.loadMessages, { conversationId }),
    saveMessage: (req: { conversationId: string; role: string; content: string }) =>
      invoke(IPC_CHANNELS.conversations.saveMessage, req),
  },
  autopilot: {
    list: () => invoke(IPC_CHANNELS.autopilot.list),
    get: (autopilotId: string) => invoke(IPC_CHANNELS.autopilot.get, { autopilotId }),
    create: (req: unknown) => invoke(IPC_CHANNELS.autopilot.create, req),
    update: (autopilotId: string, req: unknown) =>
      invoke(IPC_CHANNELS.autopilot.update, { autopilotId, ...(req as object) }),
    remove: (autopilotId: string) => invoke(IPC_CHANNELS.autopilot.remove, { autopilotId }),
    run: (autopilotId: string) => invoke(IPC_CHANNELS.autopilot.run, { autopilotId }),
    pause: (autopilotId: string) => invoke(IPC_CHANNELS.autopilot.pause, { autopilotId }),
    resume: (autopilotId: string) => invoke(IPC_CHANNELS.autopilot.resume, { autopilotId }),
  },
} as const;

contextBridge.exposeInMainWorld('api', api);

export type Api = typeof api;
