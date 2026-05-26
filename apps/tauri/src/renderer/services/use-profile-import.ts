import { useMutation, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export type ProfileImportResult =
  | { text: string; name?: string; platform: string }
  | { error: string };

export const useProfileImport = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (url: string) =>
      api.linkedin.importProfileFromUrl(url) as Promise<ProfileImportResult>,
    onSuccess: (result) => {
      if (!('error' in result)) {
        qc.invalidateQueries({ queryKey: keys.documents.all });
      }
    },
  });
};
