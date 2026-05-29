import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useCredentials = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.credentials.all, queryFn: () => api.credentials.list() });
};

export const useCredentialsAvailable = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.credentials.all, 'available'],
    queryFn: () => api.credentials.available(),
    staleTime: Infinity,
  });
};

export const useSetCredential = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { boardId: string; username: string; password: string }) =>
      api.credentials.set(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.credentials.all }),
  });
};

export const useRemoveCredential = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (boardId: string) => api.credentials.remove({ boardId }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.credentials.all }),
  });
};
