import { useMutation, useQueryClient } from '@tanstack/react-query';

import { keys } from './query-client';

export const useSignOutAll = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.privacy.signOutAll(),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.credentials.all });
      void qc.invalidateQueries({ queryKey: ['boards'] });
    },
  });
};

export const useClearInteractions = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.privacy.clearInteractions(),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.interactions() }),
  });
};
