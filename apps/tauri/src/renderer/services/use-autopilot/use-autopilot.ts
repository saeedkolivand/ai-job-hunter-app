import { useCallback, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  Autopilot,
  AutopilotCreate,
  AutopilotFocusEvent,
  AutopilotStepEvent,
  AutopilotUpdate,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';
import { useCheckBrowser } from '../use-system';

export const useAutopilots = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.autopilot.all, queryFn: () => api.autopilot.list() });
};

/**
 * Imperative "refresh the autopilots list" handle for UI components that can't
 * touch the query client directly (Ports & Adapters). Used by the `?focus`
 * deep-link consumer, which may arrive while the list is stale.
 */
export const useInvalidateAutopilots = () => {
  const qc = useQueryClient();
  return useCallback(() => {
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
  }, [qc]);
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
    // Optimistic delete: remove the card immediately, restore it if the backend fails.
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: keys.autopilot.all });
      const previous = qc.getQueryData<Autopilot[]>(keys.autopilot.all);
      qc.setQueryData<Autopilot[]>(keys.autopilot.all, (old) =>
        (old ?? []).filter((a) => a._id !== id)
      );
      return { previous };
    },
    onError: (_err, _id, ctx) => {
      if (ctx?.previous) qc.setQueryData(keys.autopilot.all, ctx.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.autopilot.all }),
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

export const useAutopilotFocusEvents = (onFocus?: (event: AutopilotFocusEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.autopilot.onFocus((event: unknown) => {
      onFocus?.(event as AutopilotFocusEvent);
    });
    return () => off?.();
  }, [api, onFocus]);
};
