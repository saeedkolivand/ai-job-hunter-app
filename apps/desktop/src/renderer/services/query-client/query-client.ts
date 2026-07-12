import { QueryClient } from '@tanstack/react-query';

/**
 * Named duration constants for React Query staleTime / gcTime / refetchInterval.
 * All values are in milliseconds. Co-locate here so every service hook references
 * the same semantic constant rather than a bare magic number.
 *
 *   SHORT         10 s  — fast-moving data (embedding status)
 *   POLLING_STALE 20 s  — paired with 30 s refetchInterval (health / metrics)
 *   MEDIUM        30 s  — default; board/bridge polling; provider key check
 *   LONG          60 s  — AI model list (changes rarely mid-session)
 *   VERY_LONG      5 min — gc default; provider models
 *   TEN_MIN       10 min — match scores; changelog (expensive, rarely stale)
 *   INFINITE       ∞    — React Query sentinel: never stale / never GC'd (session-lifetime cache)
 */
export const QUERY_TIMES = {
  SHORT: 10_000,
  POLLING_STALE: 20_000,
  MEDIUM: 30_000,
  LONG: 60_000,
  VERY_LONG: 300_000,
  TEN_MIN: 600_000,
  INFINITE: Infinity,
} as const;

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
      staleTime: QUERY_TIMES.MEDIUM,
      gcTime: QUERY_TIMES.VERY_LONG,
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
    accent: ['system', 'accent'] as const,
  },
  jobs: { all: ['jobs'] as const, detail: (id: string) => ['jobs', id] as const },
  ai: {
    models: ['ai', 'models'] as const,
    capabilities: ['ai', 'capabilities'] as const,
    embeddingStatus: ['ai', 'embeddingStatus'] as const,
    spend: ['ai', 'spend'] as const,
  },
  documents: {
    all: ['documents'] as const,
    text: (id: string) => ['documents', 'text', id] as const,
  },
  jobPreferences: { all: ['jobPreferences'] as const },
  contactProfile: { all: ['contactProfile'] as const },
  postings: {
    all: ['postings'] as const,
    interactions: (type?: string) => ['postings', 'interactions', type] as const,
    resolve: (url: string) => ['postings', 'resolve', url] as const,
  },
  credentials: { all: ['credentials'] as const },
  cliAgents: { all: ['cliAgents'] as const },
  autopilot: { all: ['autopilot'] as const, detail: (id: string) => ['autopilot', id] as const },
  aiGenerations: { all: ['aiGenerations'] as const },
  referrals: {
    all: ['referrals'] as const,
    list: (jobUrl?: string) => ['referrals', jobUrl ?? null] as const,
  },
  match: {
    score: (resumeId: string | null, jobId: string) => ['match', resumeId, jobId] as const,
  },
  updater: { changelog: ['updater', 'changelog'] as const },
  applications: {
    all: ['applications'] as const,
    detail: (id: string) => ['applications', id] as const,
  },
  extensionBridge: {
    status: ['extensionBridge', 'status'] as const,
  },
  notifications: { all: ['notifications'] as const },
  boards: { catalog: ['boards', 'catalog'] as const },
  scrapingSettings: { all: ['scrapingSettings'] as const },
} as const;
