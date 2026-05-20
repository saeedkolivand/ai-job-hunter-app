/**

 * Typed IPC contract — the single source of truth for renderer <-> main calls.

 *

 * Capability-based: each namespace is a distinct capability the preload exposes.

 * Channels are namespaced; payloads are validated by Zod in the main handlers.

 */

import type {
  AiGenerateRequest,
  AutopilotCreate,
  AutopilotUpdate,
  DocumentImportRequest,
  HybridSearchRequest,
  MatchResumeRequest,
  ScrapeBoardRequest,
  ScrapeUrlRequest,
} from '../schemas/index.js';
import type {
  AiStreamChunk,
  AppMetrics,
  Autopilot,
  CredentialMetadata,
  DocumentRecord,
  JobEvent,
  JobPosting,
  JobRecord,
  Locale,
  MatchScore,
  RuntimeHealth,
  SearchHit,
} from '../types/index.js';

export interface IpcContract {
  system: {
    health(): Promise<RuntimeHealth>;

    getVersion(): Promise<string>;

    getLocale(): Promise<Locale>;

    setLocale(locale: Locale): Promise<void>;

    getPlatform(): Promise<string>;

    openExternal(url: string): Promise<void>;

    setPerformanceMode(mode: 'low-memory' | 'balanced' | 'performance'): Promise<void>;

    getMetrics(): Promise<AppMetrics>;
  };

  jobs: {
    list(): Promise<JobRecord[]>;

    get(jobId: string): Promise<JobRecord | null>;

    cancel(jobId: string): Promise<void>;

    retry(jobId: string): Promise<void>;

    onEvent(handler: (event: JobEvent) => void): () => void;
  };

  ai: {
    generate(req: AiGenerateRequest): Promise<{ jobId: string }>;

    onStream(handler: (chunk: AiStreamChunk) => void): () => void;

    listModels(): Promise<Array<{ name: string }>>;

    pullModel(model: string): Promise<{ jobId: string }>;

    unloadModel(model: string): Promise<void>;

    /** Synchronous embedding — returns the vector. Falls back gracefully if Ollama is offline. */

    embed(req: { text: string; model?: string }): Promise<{ vector: number[]; dim: number } | null>;
  };

  documents: {
    list(): Promise<DocumentRecord[]>;

    import(req: DocumentImportRequest): Promise<{ jobId: string }>;

    remove(id: string): Promise<void>;
  };

  search: {
    hybrid(req: HybridSearchRequest): Promise<SearchHit[]>;
  };

  scrape: {
    board(req: ScrapeBoardRequest): Promise<{ jobId: string }>;

    url(req: ScrapeUrlRequest): Promise<{ jobId: string }>;

    listPostings(): Promise<JobPosting[]>;

    clearPostings(): Promise<void>;

    listInteractions(filter?: { interactionType?: string }): Promise<
      Array<{
        jobId: string;
        interactionType: string;
        timestamp: number;
        title: string;
        company: string;
        url: string;
        source: string;
        location: string;
      }>
    >;

    exportData(): Promise<{ success: boolean; filePath?: string; error?: string }>;

    importData(): Promise<{ success: boolean; imported: number; error?: string }>;
  };

  match: {
    resume(req: MatchResumeRequest): Promise<MatchScore>;
  };

  credentials: {
    /** Whether the OS supports encrypted secret storage. */

    available(): Promise<boolean>;

    /** Returns metadata only — NEVER passwords. */

    list(): Promise<CredentialMetadata[]>;

    set(req: { boardId: string; username: string; password: string }): Promise<void>;

    remove(req: { boardId: string }): Promise<void>;
  };

  linkedin: {
    /** Connect to LinkedIn by launching a browser for manual login. */

    connect(): Promise<{ connected: boolean; accountEmail?: string }>;

    /** Disconnect and clear LinkedIn session. */

    disconnect(): Promise<void>;

    /** Get current LinkedIn session status. */

    getStatus(): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;
  };

  boards: {
    /** Connect to a board by launching a browser for manual login. */

    connect(req: { boardId: string }): Promise<{ connected: boolean; accountEmail?: string }>;

    /** Disconnect a board (closes context only; does not delete profile). */

    disconnect(req: { boardId: string }): Promise<void>;

    /** Get current connection status for a board. */

    getStatus(req: {
      boardId: string;
    }): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;
  };

  privacy: {
    /** Sign out all connected accounts by wiping Chromium profiles. */

    signOutAll(): Promise<void>;

    /** Clear all saved job interaction history (applied, viewed, bookmarked). */

    clearInteractions(): Promise<void>;
  };

  apply: {
    /** Enqueue an apply.job. Returns the job id; subscribe via jobs.onEvent

     *  to watch stages/progress stream back. */

    start(req: {
      board: string;
      url: string;

      coverLetter?: string;
      resumePath?: string;
      autoSubmit?: boolean;
    }): Promise<{ jobId: string }>;

    /** List supported appliers (boardId + display name). */

    catalog(): Promise<Array<{ id: string; displayName: string }>>;
  };

  resume: {
    /** Extract plain text from an uploaded resume/job-ad file (pdf, docx, txt, md). */

    extractText(req: { name: string; bytes: Uint8Array }): Promise<{ text: string }>;
  };

  support: {
    /** Export diagnostics bundle containing system info, logs, and configuration */

    exportDiagnostics(): Promise<{ success: boolean; bundlePath?: string }>;

    /** Reload AI runtime and reload all models */

    reloadAiRuntime(): Promise<{ success: boolean }>;

    /** Unload all currently loaded AI models */

    unloadAllModels(): Promise<{ success: boolean }>;

    /** Reset all model settings to defaults */

    resetModelConfiguration(): Promise<{ success: boolean }>;

    /** Rebuild all vector indexes from scratch */

    rebuildVectorIndexes(): Promise<{ success: boolean; jobId?: string }>;

    /** Remove all cached embeddings */

    clearEmbeddingsCache(): Promise<{ success: boolean }>;

    /** Completely reset the vector database */

    resetVectorDatabase(): Promise<{ success: boolean }>;

    /** Remove all cached OCR results */

    clearOcrCache(): Promise<{ success: boolean }>;

    /** Re-index all documents in the database */

    reindexAllDocuments(): Promise<{ success: boolean; jobId?: string }>;

    /** Clear all scraping sessions and cookies */

    resetAllSessions(): Promise<{ success: boolean }>;

    /** Clear all pending scrape jobs */

    clearScrapingQueue(): Promise<{ success: boolean }>;

    /** Copy environment details to clipboard */

    copyEnvironmentDetails(): Promise<{ success: boolean }>;

    /** Copy app version to clipboard */

    copyAppVersion(): Promise<{ success: boolean }>;

    /** Copy system info to clipboard */

    copySystemInfo(): Promise<{ success: boolean }>;
  };

  conversations: {
    /** Get or create the current conversation */

    getOrCreateConversation(): Promise<{ id: string; title: string }>;

    /** Load messages for a conversation */

    loadMessages(opts: { conversationId: string }): Promise<
      Array<{
        id: string;
        role: 'user' | 'assistant' | 'system';
        content: string;
        createdAt: number;
      }>
    >;

    /** Save a single message */

    saveMessage(opts: {
      conversationId: string;
      role: 'user' | 'assistant' | 'system';
      content: string;
    }): Promise<{ success: boolean }>;

    /** Save all messages to database */

    saveAllMessages(opts: {
      conversationId: string;
      messages: Array<{
        id: string;
        role: 'user' | 'assistant' | 'system';
        content: string;
        createdAt: number;
      }>;
    }): Promise<{ success: boolean }>;
  };

  autopilot: {
    list(): Promise<Autopilot[]>;

    get(req: { autopilotId: string }): Promise<Autopilot | null>;

    create(req: AutopilotCreate): Promise<Autopilot>;

    update(req: { autopilotId: string } & AutopilotUpdate): Promise<Autopilot>;

    remove(req: { autopilotId: string }): Promise<void>;

    run(req: { autopilotId: string }): Promise<{ jobId: string }>;

    pause(req: { autopilotId: string }): Promise<void>;

    resume(req: { autopilotId: string }): Promise<void>;
  };

  updater: {
    check(): Promise<void>;

    download(): Promise<void>;

    install(): Promise<void>;

    onStatus(handler: (status: unknown) => void): () => void;
  };

  shortcuts: {
    onCommandPalette(handler: () => void): () => void;
  };
}

export const IPC_CHANNELS = {
  system: {
    health: 'system:health',

    getVersion: 'system:getVersion',

    getLocale: 'system:getLocale',

    setLocale: 'system:setLocale',

    getPlatform: 'system:getPlatform',

    openExternal: 'system:openExternal',

    setPerformanceMode: 'system:setPerformanceMode',

    getMetrics: 'system:getMetrics',
  },

  jobs: {
    list: 'jobs:list',

    get: 'jobs:get',

    cancel: 'jobs:cancel',

    retry: 'jobs:retry',

    event: 'jobs:event',
  },

  ai: {
    generate: 'ai:generate',

    stream: 'ai:stream',

    listModels: 'ai:listModels',

    pullModel: 'ai:pullModel',

    unloadModel: 'ai:unloadModel',

    embed: 'ai:embed',
  },

  documents: {
    list: 'documents:list',

    import: 'documents:import',

    remove: 'documents:remove',
  },

  search: {
    hybrid: 'search:hybrid',
  },

  scrape: {
    board: 'scrape:board',

    url: 'scrape:url',

    listPostings: 'scrape:listPostings',

    persistJob: 'scrape:persistJob',

    clearPostings: 'scrape:clearPostings',

    listInteractions: 'scrape:listInteractions',

    exportData: 'scrape:exportData',

    importData: 'scrape:importData',
  },

  match: {
    resume: 'match:resume',
  },

  credentials: {
    available: 'credentials:available',

    list: 'credentials:list',

    set: 'credentials:set',

    remove: 'credentials:remove',
  },

  linkedin: {
    connect: 'linkedin:connect',

    disconnect: 'linkedin:disconnect',

    getStatus: 'linkedin:getStatus',
  },

  boards: {
    connect: 'boards:connect',

    disconnect: 'boards:disconnect',

    getStatus: 'boards:getStatus',
  },

  apply: {
    start: 'apply:start',

    catalog: 'apply:catalog',
  },

  resume: {
    extractText: 'resume:extractText',
  },

  support: {
    exportDiagnostics: 'support:exportDiagnostics',

    reloadAiRuntime: 'support:reloadAiRuntime',

    unloadAllModels: 'support:unloadAllModels',

    resetModelConfiguration: 'support:resetModelConfiguration',

    rebuildVectorIndexes: 'support:rebuildVectorIndexes',

    clearEmbeddingsCache: 'support:clearEmbeddingsCache',

    resetVectorDatabase: 'support:resetVectorDatabase',

    clearOcrCache: 'support:clearOcrCache',

    reindexAllDocuments: 'support:reindexAllDocuments',

    resetAllSessions: 'support:resetAllSessions',

    clearScrapingQueue: 'support:clearScrapingQueue',

    copyEnvironmentDetails: 'support:copyEnvironmentDetails',

    copyAppVersion: 'support:copyAppVersion',

    copySystemInfo: 'support:copySystemInfo',
  },

  privacy: {
    signOutAll: 'privacy:signOutAll',

    clearInteractions: 'privacy:clearInteractions',
  },

  conversations: {
    getOrCreateConversation: 'conversations:getOrCreateConversation',

    loadMessages: 'conversations:loadMessages',

    saveMessage: 'conversations:saveMessage',
  },

  autopilot: {
    list: 'autopilot:list',

    get: 'autopilot:get',

    create: 'autopilot:create',

    update: 'autopilot:update',

    remove: 'autopilot:remove',

    run: 'autopilot:run',

    pause: 'autopilot:pause',

    resume: 'autopilot:resume',
  },

  updater: {
    check: 'updater:check',

    download: 'updater:download',

    install: 'updater:install',

    onStatus: 'updater:status',
  },

  shortcuts: {
    onCommandPalette: 'shortcut:command-palette',
  },
} as const;

export type IpcChannel =
  (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS][keyof (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS]];
