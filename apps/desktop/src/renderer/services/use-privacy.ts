import { useMutation, useQueryClient } from '@tanstack/react-query';
import { keys } from './query-client';

export const useSignOutAll = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.privacy.signOutAll(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.credentials.all });
      qc.invalidateQueries({ queryKey: ['boards'] });
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
