import { useMutation, useQuery } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useApplyCatalog = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.apply.catalog,
    queryFn: () => api.apply.catalog(),
    staleTime: Infinity,
  });
};

export const useApplyJob = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (req: {
      board: string;
      url: string;
      coverLetter?: string;
      resumePath?: string;
      autoSubmit?: boolean;
    }) => api.apply.start(req),
  });
};
