import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AiGenerationRecord, AiGenerationSaveRequest } from '@ajh/shared/ipc';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

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
    // Optimistic delete: drop the card immediately, restore it if the backend fails.
    onMutate: async (id) => {
      await qc.cancelQueries({ queryKey: keys.aiGenerations.all });
      const previous = qc.getQueryData<AiGenerationRecord[]>(keys.aiGenerations.all);
      qc.setQueryData<AiGenerationRecord[]>(keys.aiGenerations.all, (old) =>
        (old ?? []).filter((g) => g.id !== id)
      );
      return { previous };
    },
    onError: (_err, _id, ctx) => {
      if (ctx?.previous) qc.setQueryData(keys.aiGenerations.all, ctx.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.aiGenerations.all }),
  });
};
