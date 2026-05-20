import { QueryClient } from '@tanstack/react-query';

/**
 * Shared QueryClient for the renderer.
 *
 * Electron-specific tuning vs. web defaults:
 *   - refetchOnWindowFocus: false  — Electron windows don't "blur" like browser tabs;
 *                                    focus events fire constantly (tray, modals, dialogs).
 *   - refetchOnReconnect: false    — local-first app; IPC works without a network.
 *   - staleTime: 30s               — IPC calls are cheap but not free; no need to refetch
 *                                    on every component mount.
 *   - gcTime: 5min                 — Keep data in memory longer; avoids loading flashes
 *                                    when navigating between pages.
 *   - retry: 1                     — IPC errors are usually deterministic; one retry is enough.
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
    platform: ['system', 'platform'] as const,
  },
  jobs: { all: ['jobs'] as const, detail: (id: string) => ['jobs', id] as const },
  ai: { models: ['ai', 'models'] as const },
  documents: { all: ['documents'] as const },
  postings: {
    all: ['postings'] as const,
    interactions: (type?: string) => ['postings', 'interactions', type] as const,
  },
  search: { results: (q: string) => ['search', q] as const },
  credentials: { all: ['credentials'] as const },
  autopilot: { all: ['autopilot'] as const, detail: (id: string) => ['autopilot', id] as const },
  conversations: { detail: (id: string) => ['conversations', id] as const },
  apply: { catalog: ['apply', 'catalog'] as const },
} as const;
