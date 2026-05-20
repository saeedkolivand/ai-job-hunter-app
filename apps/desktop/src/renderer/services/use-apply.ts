import { useQuery, useMutation } from '@tanstack/react-query';
import { keys } from './query-client';

export const useApplyCatalog = () =>
  useQuery({
    queryKey: keys.apply.catalog,
    queryFn: () => window.api.apply.catalog(),
    staleTime: Infinity, // board catalog is static at runtime
  });

export const useApplyJob = () =>
  useMutation({
    mutationFn: (req: {
      board: string;
      url: string;
      coverLetter?: string;
      resumePath?: string;
      autoSubmit?: boolean;
    }) => window.api.apply.start(req),
  });
