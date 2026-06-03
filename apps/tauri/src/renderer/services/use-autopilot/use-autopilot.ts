import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AutopilotCreate, AutopilotStepEvent, AutopilotUpdate } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';
import { useCheckBrowser } from '../use-system';

export const useAutopilots = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.autopilot.all, queryFn: () => api.autopilot.list() });
};

export const useAutopilot = (id: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.autopilot.detail(id),
    queryFn: () => api.autopilot.get({ autopilotId: id }),
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
      api.autopilot.update({ autopilotId: id, ...data }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRemoveAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.remove({ autopilotId: id }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useRunAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const { data: browserCheck } = useCheckBrowser();

  return useMutation({
    mutationFn: async (id: string) => {
      if (!browserCheck?.detected) {
        throw new Error(
          'Chrome or Edge is required for autopilot job applications. Please install Chrome or Edge, or set the CHROME environment variable to point to your browser installation.'
        );
      }
      return api.autopilot.run({ autopilotId: id });
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const usePauseAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.pause({ autopilotId: id }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useResumeAutopilot = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.autopilot.resume({ autopilotId: id }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
  });
};

export const useAutopilotStepEvents = (onStep?: (event: AutopilotStepEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.autopilot.onStep((event: unknown) => {
      onStep?.(event as AutopilotStepEvent);
    });
    return () => off?.();
  }, [api, onStep]);
};
