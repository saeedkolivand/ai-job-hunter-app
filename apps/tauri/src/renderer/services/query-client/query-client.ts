import { QueryClient } from '@tanstack/react-query';

/**
 * Shared QueryClient for the renderer.
 *
 * Tuned for a local-first desktop app (applies equally to Electron, Tauri, or a
 * future web deployment backed by a local runtime):
 *   - refetchOnWindowFocus: false  — desktop windows receive spurious focus events
 *                                    (tray clicks, modals, dialogs) that would cause
 *                                    unnecessary refetches on every interaction.
 *   - refetchOnReconnect: false    — data comes from the local runtime, not a remote
 *                                    server; a "reconnect" event is not meaningful.
 *   - staleTime: 30s               — backend calls are cheap but not free; avoids
 *                                    redundant fetches on every component mount.
 *   - gcTime: 5min                 — keep cached data longer to prevent loading flashes
 *                                    when navigating between pages.
 *   - retry: 1                     — backend errors are usually deterministic;
 *                                    one retry is sufficient.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
      retry: 1,
    },
    mutations: {
      retry: 0,
    },
  },
});

/** Query key factory — keeps cache keys consistent and co-located. */
export const keys = {
  system: {
    health: ['system', 'health'] as const,
    version: ['system', 'version'] as const,
    protocolVersion: ['system', 'protocolVersion'] as const,
    platform: ['system', 'platform'] as const,
    metrics: ['system', 'metrics'] as const,
    checkBrowser: ['system', 'checkBrowser'] as const,
    launchAtLogin: ['system', 'launchAtLogin'] as const,
  },
  jobs: { all: ['jobs'] as const, detail: (id: string) => ['jobs', id] as const },
  ai: {
    models: ['ai', 'models'] as const,
    embeddingStatus: ['ai', 'embeddingStatus'] as const,
  },
  documents: { all: ['documents'] as const },
  jobPreferences: { all: ['jobPreferences'] as const },
  contactProfile: { all: ['contactProfile'] as const },
  postings: {
    all: ['postings'] as const,
    interactions: (type?: string) => ['postings', 'interactions', type] as const,
    resolve: (url: string) => ['postings', 'resolve', url] as const,
  },
  search: { results: (q: string) => ['search', q] as const },
  credentials: { all: ['credentials'] as const },
  cliAgents: { all: ['cliAgents'] as const },
  autopilot: { all: ['autopilot'] as const, detail: (id: string) => ['autopilot', id] as const },
  aiGenerations: { all: ['aiGenerations'] as const },
  referrals: {
    all: ['referrals'] as const,
    list: (jobUrl?: string) => ['referrals', jobUrl ?? null] as const,
  },
  match: { score: (resumeId: string | null, jobId: string) => ['match', resumeId, jobId] as const },
  updater: { changelog: ['updater', 'changelog'] as const },
  applications: {
    all: ['applications'] as const,
    detail: (id: string) => ['applications', id] as const,
  },
  extensionBridge: {
    status: ['extensionBridge', 'status'] as const,
  },
  notifications: { all: ['notifications'] as const },
} as const;
