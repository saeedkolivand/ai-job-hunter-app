/**
 * createMockClient — factory for a fully-stubbed AppClient.
 *
 * Intended for:
 *   • Vitest / Jest unit tests of renderer components and service hooks.
 *   • Storybook stories that need a client but no backend.
 *   • A future web HTTP adapter: start with stubs, replace one namespace at a
 *     time with real fetch calls until parity is reached.
 *
 * Usage:
 *   const client = createMockClient();
 *   // override individual methods for a specific test:
 *   const client = createMockClient({
 *     system: { health: async () => ({ status: 'ok' }) },
 *   });
 *
 * Every method is a jest/vitest spy-friendly async stub. Provide overrides as a
 * deep-partial — only the methods you care about need to be specified.
 */
import type { Locale, SearchHit } from '@ajh/shared/types';

import type { AppClient } from '../app-client';

type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};

const noop = () => Promise.resolve() as Promise<never>;
const emptyList = () => Promise.resolve([]) as Promise<never>;
const unsub = () => () => {};

export function createMockClient(overrides: DeepPartial<AppClient> = {}): AppClient {
  const base: AppClient = {
    system: {
      health: noop,
      getVersion: noop,
      getLocale: async () => 'en' as Locale,
      setLocale: noop,
      getPlatform: noop,
      openExternal: noop,
      setPerformanceMode: noop,
      getMetrics: noop,
      checkBrowser: async () => ({ detected: false }),
      openDevtools: noop,
      getProtocolVersion: async () => '1.0.0',
    },

    jobs: {
      list: emptyList,
      get: noop,
      cancel: noop,
      retry: noop,
      onEvent: unsub,
    },

    ai: {
      generate: noop,
      generatePipeline: noop,
      listModels: emptyList,
      pullModel: noop,
      unloadModel: noop,
      embed: noop,
      onStream: unsub,
      setProviderKey: noop,
      removeProviderKey: noop,
      hasProviderKey: async () => ({ has: false }),
      testProviderKey: async () => ({ success: true }),
      listProviderModels: emptyList,
      embeddingStatus: async () => ({
        active: { provider: 'ollama', model: 'nomic-embed-text' },
        spaces: [],
        documents: { total: 0, indexedInActiveSpace: 0, stale: 0 },
      }),
      setEmbeddingConfig: async () => ({ success: true }),
      reembedAll: async () => ({ jobId: 'mock-reembed' }),
    },

    aiGenerations: {
      list: emptyList,
      save: noop,
      remove: noop,
    },

    documents: {
      list: emptyList,
      import: noop,
      recommendTemplate: async () => ({
        templateId: 'modern',
        locale: 'en',
        atsSuggested: false,
        rationale: 'Mock recommendation.',
      }),
      remove: noop,
      setDefault: noop,
      exportDocument: async () => ({ data: [], mimeType: 'text/plain', filename: 'mock.txt' }),
      exportAndSave: noop,
    },

    jobPreferences: {
      get: async () => ({}),
      set: noop,
    },

    search: {
      hybrid: async () => [] as SearchHit<unknown>[],
    },

    scrape: {
      board: noop,
      url: noop,
      resolveUrl: async () => null,
      persistJob: noop,
      listPostings: emptyList,
      clearPostings: noop,
      listInteractions: emptyList,
    },
    data: {
      export: async () => ({ success: false }),
      import: async () => ({ success: false }),
    },

    match: {
      resume: noop,
    },

    geocode: {
      suggest: async () => [],
    },

    credentials: {
      available: async () => false,
      list: emptyList,
      set: noop,
      remove: noop,
    },

    linkedin: {
      connect: noop,
      disconnect: noop,
      getStatus: async () => ({ connected: false }),
      importProfileFromUrl: async () => ({ error: 'not available in mock' }),
    },

    boards: {
      connect: async () => ({ connected: false }),
      disconnect: noop,
      getStatus: async () => ({ connected: false }),
    },

    privacy: {
      signOutAll: noop,
      clearInteractions: noop,
      resetApp: noop,
    },

    apply: {
      start: noop,
      catalog: emptyList,
    },

    updater: {
      check: noop,
      download: noop,
      install: noop,
      changelog: () => Promise.resolve({ releases: [] }),
      onStatus: unsub,
    },

    shortcuts: {
      onCommandPalette: unsub,
    },

    resume: {
      extractText: noop,
    },

    support: {
      exportDiagnostics: noop,
      reloadAiRuntime: noop,
      unloadAllModels: noop,
      resetModelConfiguration: noop,
      rebuildVectorIndexes: noop,
      clearEmbeddingsCache: noop,
      resetVectorDatabase: noop,
      clearOcrCache: noop,
      reindexAllDocuments: noop,
      resetAllSessions: noop,
      clearScrapingQueue: noop,
      copyEnvironmentDetails: noop,
      copyAppVersion: noop,
      copySystemInfo: noop,
    },

    conversations: {
      getOrCreateConversation: async () => ({ id: 'mock', title: 'Mock' }),
      loadMessages: emptyList,
      saveMessage: noop,
      saveAllMessages: noop,
    },

    autopilot: {
      list: emptyList,
      get: noop,
      create: noop,
      update: noop,
      remove: noop,
      run: noop,
      pause: noop,
      resume: noop,
      onStep: () => () => {},
    },

    dialog: {
      openFiles: async () => [] as string[],
    },
  };

  // Shallow-merge overrides at the namespace level.
  for (const ns of Object.keys(overrides) as Array<string & keyof AppClient>) {
    if (overrides[ns]) {
      (base as unknown as Record<string, unknown>)[ns] = {
        ...(base[ns] as object),
        ...overrides[ns],
      };
    }
  }

  return base;
}
