import type { AppClient } from '../../app-client';
import { emptyList, noop, unsub } from './helpers';

export const coreNamespaces = (): Pick<AppClient, 'system' | 'jobs' | 'ai' | 'aiGenerations'> => ({
  system: {
    health: noop,
    getVersion: noop,
    getLocale: async () => 'en',
    setLocale: noop,
    getPlatform: noop,
    accentColor: async () => ({ supported: false, color: null }),
    openExternal: noop,
    // Accepts the resolved PerformanceBackendConfig; no-op stub for tests.
    setPerformanceMode: noop,
    getLaunchAtLogin: async () => false,
    setLaunchAtLogin: async (enabled: boolean) => enabled,
    setCloseToTray: noop,
    getMetrics: noop,
    checkBrowser: async () => ({ detected: false }),
    openDevtools: noop,
    getProtocolVersion: async () => '1.1.0',
    // A path with a SPACE in it, on purpose: every consumer of this value
    // has to quote it (a shell command, a TOML value), and a space-free stub
    // would let an unquoted snippet pass Storybook and e2e unnoticed.
    agentCliInfo: async () => ({
      exePath: 'C:\\Users\\demo\\AppData\\Local\\AI Job Hunter\\ajh-tauri.exe',
    }),
    onAccentChanged: unsub,
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
    inspectModel: async () => null,
    activeConfig: async () => ({ providers: {} }),
    setActiveProvider: async () => ({ providers: {} }),
    setProviderSettings: async () => ({ providers: {} }),
    seedActiveConfig: async () => ({ seeded: false }),
    researchCompany: async () => ({ company: '', brief: '' }),
    lookupSalary: async () => null,
    researchAnswer: async () => '',
    pullModel: noop,
    unloadModel: noop,
    embed: noop,
    onStream: unsub,
    setProviderKey: noop,
    removeProviderKey: noop,
    hasProviderKey: async () => ({ has: false }),
    testProviderKey: async () => ({ success: true }),
    listProviderModels: emptyList,
    modelCapabilities: async () => ({
      supportsWebSearch: false,
      supportsReasoning: false,
      effortLevels: [],
    }),
    embeddingStatus: async () => ({
      active: { provider: 'ollama', model: 'nomic-embed-text' },
      spaces: [],
      documents: { total: 0, indexedInActiveSpace: 0, stale: 0 },
      indexing: false,
    }),
    setEmbeddingConfig: async () => ({ success: true }),
    reembedAll: async () => ({ jobId: 'mock-reembed' }),
    indexStaleDocuments: async () => ({ jobId: null }),
    stageOverrides: async () => ({}),
    setStageOverride: async () => ({}),
    clearStageOverride: async () => ({}),
    // Echo the requested window so a multi-day caller can be exercised against the mock.
    spendSummary: async (days = 1) => ({
      window: { days, from: 0, to: 0 },
      today: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
      windowTotals: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
      perProvider: [],
      thinkingByModel: [],
      thinkingByModelWindow: 'allTime',
    }),
  },

  aiGenerations: {
    list: emptyList,
    save: noop,
    update: noop,
    remove: noop,
    removeBulk: noop,
  },
});
