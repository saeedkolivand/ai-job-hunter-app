import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ContactProfile } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useContactProfile = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.contactProfile.all,
    queryFn: () => api.contactProfile.get(),
  });
};

export const useSaveContactProfile = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (profile: ContactProfile) => api.contactProfile.set(profile),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.contactProfile.all }),
  });
};
