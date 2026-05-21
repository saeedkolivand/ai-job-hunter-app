import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AutopilotCreate, AutopilotUpdate } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export const useAutopilots = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.autopilot.all, queryFn: () => api.autopilot.list() });
};

export const useAutopilot = (id: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.autopilot.detail(id),
    queryFn: () => api.autopilot.get(id),
    enabled: !!id,
  });
};

export const useCreateAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AutopilotCreate) => api.autopilot.create(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useUpdateAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...data }: { id: string } & AutopilotUpdate) =>
      api.autopilot.update(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRemoveAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.remove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRunAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.run(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const usePauseAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.pause(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useResumeAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.resume(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};
