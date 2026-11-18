import { useQuery } from '@tanstack/react-query';

import type { AtsView } from '@ajh/shared/ipc';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * The strict "what an ATS sees" view of a previously exported PDF/DOCX at `path`.
 * Disabled until a path is given. Re-exporting overwrites the same path, so the view
 * is always stale and re-read when the path is set again (the export clears it first),
 * but not on window focus; a failed read (bad path, unreadable file) is not retried.
 */
export const useAtsView = (path: string | null) => {
  const api = useAppClient();
  return useQuery<AtsView>({
    queryKey: keys.export.atsView(path ?? ''),
    queryFn: () => api.export.atsView(path as string),
    enabled: Boolean(path),
    staleTime: 0,
    // No cached view survives unmount, so a re-export never flashes the previous file's reading.
    gcTime: 0,
    refetchOnWindowFocus: false,
    retry: false,
  });
};
