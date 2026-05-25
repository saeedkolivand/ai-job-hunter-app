import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AiGenerationSaveRequest } from '@ajh/shared/ipc';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export const useAiGenerations = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.aiGenerations.all,
    queryFn: () => api.aiGenerations.list(),
  });
};

export const useSaveAiGeneration = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AiGenerationSaveRequest) => api.aiGenerations.save(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.aiGenerations.all }),
  });
};

export const useRemoveAiGeneration = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.aiGenerations.remove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.aiGenerations.all }),
  });
};
