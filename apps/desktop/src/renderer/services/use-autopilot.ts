import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AutopilotCreate, AutopilotUpdate } from '@ajh/shared';

import { keys } from './query-client';

export const useAutopilots = () =>
  useQuery({
    queryKey: keys.autopilot.all,
    queryFn: () => window.api.autopilot.list(),
  });

export const useAutopilot = (id: string) =>
  useQuery({
    queryKey: keys.autopilot.detail(id),
    // preload signature: get(autopilotId: string)
    queryFn: () => window.api.autopilot.get(id),
    enabled: !!id,
  });

export const useCreateAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AutopilotCreate) => window.api.autopilot.create(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useUpdateAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    // preload signature: update(autopilotId: string, req: unknown)
    mutationFn: ({ id, ...data }: { id: string } & AutopilotUpdate) =>
      window.api.autopilot.update(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRemoveAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    // preload signature: remove(autopilotId: string)
    mutationFn: (id: string) => window.api.autopilot.remove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRunAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => window.api.autopilot.run(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const usePauseAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => window.api.autopilot.pause(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useResumeAutopilot = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => window.api.autopilot.resume(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};
