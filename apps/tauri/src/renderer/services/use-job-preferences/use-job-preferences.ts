import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { JobPreferences } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useJobPreferences = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.jobPreferences.all,
    queryFn: () => api.jobPreferences.get(),
  });
};

export const useSetJobPreferences = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (prefs: JobPreferences) => api.jobPreferences.set(prefs),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobPreferences.all }),
  });
};
