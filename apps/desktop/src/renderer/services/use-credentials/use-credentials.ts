import { useQuery } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useCredentialsAvailable = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.credentials.all, 'available'],
    queryFn: () => api.credentials.available(),
    staleTime: Infinity,
  });
};
