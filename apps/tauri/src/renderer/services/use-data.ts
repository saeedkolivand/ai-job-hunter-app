import { useMutation, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

/** Export all app data to a user-chosen JSON file. */
export const useExportData = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.data.export() });
};

/** Restore all app data from a backup file, then refresh every cached query. */
export const useImportData = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.data.import(),
    onSuccess: () => qc.invalidateQueries(),
  });
};
