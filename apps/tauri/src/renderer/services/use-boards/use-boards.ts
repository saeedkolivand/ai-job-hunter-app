import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

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
    refetchInterval: 30_000,
  });
};

export const useLinkedInConnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const { data: browserCheck } = useCheckBrowser();

  return useMutation({
    mutationFn: async () => {
      if (!browserCheck?.detected) {
        throw new Error(
          'Chrome or Edge is required for LinkedIn login. Please install Chrome or Edge, or set the CHROME environment variable to point to your browser installation.'
        );
      }
      return api.linkedin.connect();
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
    onSettled: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
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
    refetchInterval: 30_000,
    enabled: !!boardId,
  });
};

export const useBoardConnect = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const { data: browserCheck } = useCheckBrowser();

  return useMutation({
    mutationFn: async (boardId: string) => {
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
