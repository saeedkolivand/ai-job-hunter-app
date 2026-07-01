import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  Application,
  ApplicationChangedEvent,
  ApplicationTrackRequest,
  ApplicationUpdateRequest,
} from '@ajh/shared';

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

/**
 * App-global subscription to out-of-band application changes (`applications:changed`),
 * emitted by the browser-extension bridge on a successful import. Invalidates the
 * applications list and the postings list so both refresh live.
 *
 * Mounted ONCE in the root layout (like `useMenuNavigation`); never call from a
 * feature component, or the listener would attach/detach per route.
 */
export const useApplicationEvents = (onChanged?: (event: ApplicationChangedEvent) => void) => {
  const api = useAppClient();
  const qc = useQueryClient();
  // Keep the latest handler in a ref so the listener subscribes ONCE — re-subscribing
  // on every render races the async Tauri `listen` and can drop an event in the gap.
  const handlerRef = useRef(onChanged);
  handlerRef.current = onChanged;
  useEffect(() => {
    const off = api.applications.onChanged((event) => {
      void qc.invalidateQueries({ queryKey: keys.applications.all });
      // A new application created from a posting flips that posting's "applied" badge,
      // so the postings list must refresh too.
      void qc.invalidateQueries({ queryKey: keys.postings.all });
      handlerRef.current?.(event);
    });
    return () => off();
  }, [api, qc]);
};
