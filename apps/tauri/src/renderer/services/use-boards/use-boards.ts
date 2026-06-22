import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';
import { useCheckBrowser } from '../use-system';

const KEYS = {
  linkedinStatus: ['boards', 'linkedin', 'status'] as const,
  boardStatus: (boardId: string) => ['boards', boardId, 'status'] as const,
};

// ─── LinkedIn ─────────────────────────────────────────────────────────────────

export const useLinkedInStatus = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: KEYS.linkedinStatus,
    queryFn: () => api.linkedin.getStatus(),
    refetchInterval: QUERY_TIMES.MEDIUM,
  });
};

export const useLinkedInConnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const { data: browserCheck } = useCheckBrowser();

  return useMutation({
    mutationFn: async () => {
      // Try cookie import first — avoids opening a browser window when the
      // user already has an active session in Chrome/Edge.
      const importResult = await api.linkedin.importCookies();
      if (importResult.outcome === 'Imported') {
        return { connected: true, viaImport: true };
      }

      // Fall back to browser-window login.
      if (!browserCheck?.detected) {
        throw new Error(
          'Chrome or Edge is required for LinkedIn login. Please install Chrome or Edge, or set the CHROME environment variable to point to your browser installation.'
        );
      }
      return api.linkedin.connect();
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
  });
};

export const useLinkedInDisconnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.linkedin.disconnect(),
    onMutate: () => {
      // Optimistic update — show disconnected immediately before the round-trip.
      qc.setQueryData(KEYS.linkedinStatus, { connected: false });
    },
    onSettled: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
  });
};

// ─── Generic boards ──────────────────────────────────────────────────────────

export const useBoardStatus = (boardId: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: KEYS.boardStatus(boardId),
    queryFn: () => api.boards.getStatus({ boardId }),
    refetchInterval: QUERY_TIMES.MEDIUM,
    enabled: !!boardId,
  });
};

export const useBoardConnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const { data: browserCheck } = useCheckBrowser();

  return useMutation({
    mutationFn: async (boardId: string) => {
      // Try cookie import first — avoids opening a browser window when the
      // user already has an active session in Chrome/Edge.
      const importResult = await api.boards.importCookies({ boardId });
      if (importResult.outcome === 'Imported') {
        return { connected: true, viaImport: true };
      }

      // Fall back to browser-window login.
      if (!browserCheck?.detected) {
        throw new Error(
          'Chrome or Edge is required for job board login. Please install Chrome or Edge, or set the CHROME environment variable to point to your browser installation.'
        );
      }
      return api.boards.connect({ boardId });
    },
    onSuccess: (_data, boardId) => qc.invalidateQueries({ queryKey: KEYS.boardStatus(boardId) }),
  });
};

export const useBoardDisconnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (boardId: string) => api.boards.disconnect({ boardId }),
    onMutate: (boardId) => {
      qc.setQueryData(KEYS.boardStatus(boardId), { connected: false });
    },
    onSettled: (_data, _err, boardId) =>
      qc.invalidateQueries({ queryKey: KEYS.boardStatus(boardId) }),
  });
};

// ─── Multi-board status ───────────────────────────────────────────────────────

/**
 * Query connection status for a dynamic list of board IDs in parallel.
 *
 * Uses `useQueries` (the Rules-of-Hooks-safe API for dynamic query arrays).
 * LinkedIn uses its own status key; all other boards use the generic board key.
 * Returns `true` if at least one queried board reports `connected: true`.
 */
export const useBoardStatuses = (boardIds: string[] = []) => {
  const api = useAppClient();
  const safeIds = Array.isArray(boardIds) ? boardIds : [];
  const results = useQueries({
    queries: safeIds.map((id) =>
      id === 'linkedin'
        ? {
            queryKey: KEYS.linkedinStatus,
            queryFn: () => api.linkedin.getStatus(),
            refetchInterval: QUERY_TIMES.MEDIUM,
          }
        : {
            queryKey: KEYS.boardStatus(id),
            queryFn: () => api.boards.getStatus({ boardId: id }),
            refetchInterval: QUERY_TIMES.MEDIUM,
            enabled: !!id,
          }
    ),
  });
  const anyConnected = results.some(
    (r) => (r.data as { connected?: boolean } | undefined)?.connected === true
  );

  return { results, anyConnected };
};

// ─── Catalog ─────────────────────────────────────────────────────────────────

/** Board catalog — static for the session (registry order, listed boards only via filter). */
export const useBoardsCatalog = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.boards.catalog,
    queryFn: () => api.boards.catalog(),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
};
