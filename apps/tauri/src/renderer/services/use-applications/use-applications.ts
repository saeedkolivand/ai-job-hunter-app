import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { Application, ApplicationTrackRequest, ApplicationUpdateRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useApplications = () => {
  const api = useAppClient();
  return useQuery<Application[]>({
    queryKey: keys.applications.all,
    queryFn: () => api.applications.list(),
  });
};

export const useApplication = (id: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.applications.detail(id),
    queryFn: () => api.applications.get(id),
    enabled: !!id,
  });
};

export const useSetApplicationStatus = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, status, note }: { id: string; status: string; note?: string }) =>
      api.applications.setStatus({ id, status, note }),
    onSuccess: (_data, { id }) => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
      void qc.invalidateQueries({ queryKey: keys.applications.detail(id) });
    },
  });
};

export const useUpdateApplication = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ApplicationUpdateRequest) => api.applications.update(req),
    onSuccess: (_data, req) => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
      void qc.invalidateQueries({ queryKey: keys.applications.detail(req.id) });
    },
  });
};

export const useRemoveApplication = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, keepDocuments }: { id: string; keepDocuments: boolean }) =>
      api.applications.remove({ id, keepDocuments }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
    },
  });
};

export const useTrackApplication = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ApplicationTrackRequest) => api.applications.track(req),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
    },
  });
};

export const useSaveFromPosting = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ApplicationTrackRequest) => api.applications.saveFromPosting(req),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
    },
  });
};
