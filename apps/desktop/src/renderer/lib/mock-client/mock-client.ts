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
import type { ReferralContact, ReferralUpsertRequest, ScrapeProgressEvent } from '@ajh/shared';

import type { AppClient } from '../app-client';

type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};

const noop = () => Promise.resolve() as Promise<never>;
const emptyList = () => Promise.resolve([]) as Promise<never>;
const unsub = () => () => {};

// Where the mock scrape namespace stashes its progress emitter. Off-contract
// (ScrapeContract has no emit surface), so a symbol keeps it out of enumeration
// and object-spread merges — tests reach it via `emitScrapeProgress`.
const SCRAPE_PROGRESS_EMITTER = Symbol('scrapeProgressEmitter');

type ScrapeProgressEmitting = {
  [SCRAPE_PROGRESS_EMITTER]?: (event: ScrapeProgressEvent) => void;
};

export function createMockClient(overrides: DeepPartial<AppClient> = {}): AppClient {
  // In-memory referral store so the renderer/tests can exercise list/upsert/remove
  // offline without a backend. Scoped per client so each mock starts empty.
  const referralRows: ReferralContact[] = [];

  // In-memory scrape-progress fan-out so tests can drive the onProgress path
  // (register a handler, then push events via `emitScrapeProgress`). Scoped per
  // client so each mock starts with no subscribers.
  const scrapeProgressHandlers = new Set<(event: ScrapeProgressEvent) => void>();

  const base: AppClient = {
    agent: {
      run: async () => ({ jobId: 'mock-agent' }),
      confirm: async () => ({ ok: true }),
      onStep: unsub,
    },

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
      modelCapabilities: async () => ({ supportsWebSearch: false }),
      embeddingStatus: async () => ({
        active: { provider: 'ollama', model: 'nomic-embed-text' },
        spaces: [],
        documents: { total: 0, indexedInActiveSpace: 0, stale: 0 },
      }),
      setEmbeddingConfig: async () => ({ success: true }),
      reembedAll: async () => ({ jobId: 'mock-reembed' }),
      spendSummary: async () => ({
        today: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
        perProvider: [],
      }),
    },

    aiGenerations: {
      list: emptyList,
      save: noop,
      update: noop,
      remove: noop,
      removeBulk: noop,
    },

    applications: {
      list: emptyList,
      get: async () => ({ application: null, events: [] }),
      setStatus: async () => ({ success: true }),
      update: async () => ({ success: true }),
      remove: async () => ({ success: true }),
      track: async () => ({ success: true }),
      saveFromPosting: async () => ({ success: true }),
      onChanged: unsub,
    },

    documents: {
      list: emptyList,
      getText: async () => '',
      import: noop,
      recommendTemplate: async () => ({
        templateId: 'classic',
        locale: 'en',
        atsSuggested: false,
        rationale: 'Mock recommendation.',
      }),
      remove: noop,
      setDefault: noop,
      exportDocument: async () => ({ data: [], mimeType: 'text/plain', filename: 'mock.txt' }),
      exportAndSave: noop,
      renderPreviewImages: async () => ({ pages: [], mimeType: 'image/svg+xml' }),
    },

    jobPreferences: {
      get: async () => ({}),
      set: noop,
    },

    contactProfile: {
      get: async () => ({}),
      set: async () => ({ success: true }),
    },

    github: {
      importRepos: emptyList,
    },

    extensionBridge: {
      status: async () => ({ port: 47615, connected: false, token: 'mock-token' }),
      regenerateToken: async () => ({ token: 'mock-token' }),
      autofillEnabled: async () => ({ enabled: false }),
      setAutofillEnabled: async (enabled: boolean) => ({ enabled }),
      aiAssistEnabled: async () => ({ enabled: false }),
      setAiAssistEnabled: async (enabled: boolean) => ({ enabled }),
    },

    scrape: {
      boards: noop,
      url: noop,
      resolveUrl: async () => null,
      updateDescription: async () => false,
      persistJob: noop,
      listPostings: emptyList,
      clearPostings: noop,
      listInteractions: emptyList,
      onProgress: (handler) => {
        scrapeProgressHandlers.add(handler);
        return () => {
          scrapeProgressHandlers.delete(handler);
        };
      },
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
    },

    linkedin: {
      connect: noop,
      disconnect: noop,
      getStatus: async () => ({ connected: false }),
      importProfileFromUrl: async () => ({ error: 'not available in mock' }),
      importCookies: async () => ({ outcome: 'NoSession', imported: 0 }),
    },

    boards: {
      catalog: async () => [],
      connect: async () => ({ connected: false }),
      disconnect: noop,
      getStatus: async () => ({ connected: false }),
      importCookies: async () => ({ outcome: 'NoSession', imported: 0 }),
    },

    cliAgents: {
      status: async () => ({ agents: [], npmAvailable: false }),
      redetect: async () => ({ agents: [], npmAvailable: false }),
      install: async () => ({ code: 0, success: true }),
    },

    privacy: {
      signOutAll: noop,
      clearInteractions: noop,
      resetApp: async () => ({ success: true }),
    },

    referrals: {
      list: async (jobUrl?: string) =>
        jobUrl ? referralRows.filter((r) => r.jobUrl === jobUrl) : [...referralRows],
      upsert: async (req: ReferralUpsertRequest) => {
        const now = Date.now();
        const existing = req.id ? referralRows.find((r) => r.id === req.id) : undefined;
        const record: ReferralContact = {
          id: existing?.id ?? req.id ?? `ref-${now}-${Math.random().toString(36).slice(2, 10)}`,
          jobUrl: req.jobUrl ?? '',
          companyName: req.companyName ?? '',
          personName: req.personName ?? '',
          personRole: req.personRole ?? '',
          linkedinUrl: req.linkedinUrl ?? '',
          emailDraft: req.emailDraft ?? '',
          messageDraft: req.messageDraft ?? '',
          inviteNoteDraft: req.inviteNoteDraft ?? '',
          channel: req.channel ?? 'email',
          status: req.status ?? 'draft',
          notes: req.notes ?? '',
          createdAt: existing?.createdAt ?? now,
          updatedAt: now,
        };
        if (existing) {
          referralRows.splice(referralRows.indexOf(existing), 1, record);
        } else {
          referralRows.push(record);
        }
        return record;
      },
      remove: async (id: string) => {
        const i = referralRows.findIndex((r) => r.id === id);
        if (i >= 0) referralRows.splice(i, 1);
      },
    },

    updater: {
      check: noop,
      download: noop,
      install: noop,
      changelog: () => Promise.resolve({ releases: [] }),
      onStatus: unsub,
    },

    resume: {
      extractText: noop,
    },

    support: {
      exportDiagnostics: noop,
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
      onFocus: () => () => {},
      takePendingFocus: () => Promise.resolve(null),
    },

    menu: {
      onNavigate: unsub,
      onAction: unsub,
      takePending: () => Promise.resolve(null),
    },

    notifications: {
      list: emptyList,
      markRead: noop,
      markAllRead: noop,
      remove: noop,
      clearAll: noop,
      clicked: noop,
      onChanged: unsub,
      onOpenInbox: unsub,
      onOsBannerClick: unsub,
      onToast: unsub,
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

  // Attach the progress emitter after merging so it survives a `scrape` override
  // (which replaces the namespace object). Fans an event out to every handler
  // currently registered via `scrape.onProgress`.
  (base.scrape as unknown as ScrapeProgressEmitting)[SCRAPE_PROGRESS_EMITTER] = (event) => {
    for (const handler of scrapeProgressHandlers) handler(event);
  };

  return base;
}

/**
 * Push a scrape-progress event to every handler registered via
 * `client.scrape.onProgress` on a mock client (see {@link createMockClient}).
 * Lets renderer tests exercise the scrape-progress path without a backend.
 * No-op on any client that isn't a mock.
 */
export function emitScrapeProgress(client: AppClient, event: ScrapeProgressEvent): void {
  (client.scrape as unknown as ScrapeProgressEmitting)[SCRAPE_PROGRESS_EMITTER]?.(event);
}
