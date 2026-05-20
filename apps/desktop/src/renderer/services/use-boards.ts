import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

const KEYS = {
  linkedinStatus: ['boards', 'linkedin', 'status'] as const,
  boardStatus: (boardId: string) => ['boards', boardId, 'status'] as const,
};

// ─── LinkedIn ─────────────────────────────────────────────────────────────────

export const useLinkedInStatus = () =>
  useQuery({
    queryKey: KEYS.linkedinStatus,
    queryFn: () => window.api.linkedin.getStatus(),
    refetchInterval: 30_000,
  });

export const useLinkedInConnect = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.linkedin.connect(),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
  });
};

export const useLinkedInDisconnect = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.linkedin.disconnect(),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEYS.linkedinStatus }),
  });
};

// ─── Generic boards ──────────────────────────────────────────────────────────

export const useBoardStatus = (boardId: string) =>
  useQuery({
    queryKey: KEYS.boardStatus(boardId),
    queryFn: () => window.api.boards.getStatus(boardId),
    refetchInterval: 30_000,
    enabled: !!boardId,
  });

export const useBoardConnect = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (boardId: string) => window.api.boards.connect(boardId),
    onSuccess: (_data, boardId) => qc.invalidateQueries({ queryKey: KEYS.boardStatus(boardId) }),
  });
};

export const useBoardDisconnect = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (boardId: string) => window.api.boards.disconnect(boardId),
    onSuccess: (_data, boardId) => qc.invalidateQueries({ queryKey: KEYS.boardStatus(boardId) }),
  });
};
