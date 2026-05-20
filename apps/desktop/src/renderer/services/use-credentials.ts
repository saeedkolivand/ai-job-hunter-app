import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { keys } from './query-client';

export const useCredentials = () =>
  useQuery({
    queryKey: keys.credentials.all,
    queryFn: () => window.api.credentials.list(),
  });

export const useCredentialsAvailable = () =>
  useQuery({
    queryKey: [...keys.credentials.all, 'available'],
    queryFn: () => window.api.credentials.available(),
    staleTime: Infinity,
  });

export const useSetCredential = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { boardId: string; username: string; password: string }) =>
      window.api.credentials.set(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.credentials.all }),
  });
};

export const useRemoveCredential = () => {
  const qc = useQueryClient();
  return useMutation({
    // preload signature: remove(boardId: string)
    mutationFn: (boardId: string) => window.api.credentials.remove(boardId),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.credentials.all }),
  });
};
