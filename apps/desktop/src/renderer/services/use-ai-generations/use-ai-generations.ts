import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  AiGenerationRecord,
  AiGenerationSaveRequest,
  AiGenerationUpdateRequest,
} from '@ajh/shared/ipc';

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

export const useUpdateAiGeneration = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AiGenerationUpdateRequest) => api.aiGenerations.update(req),
    // Optimistic edit: patch the matching record's text in place so the UI
    // reflects the edit immediately; restore the snapshot if the backend fails.
    // Patching (rather than refetching) avoids clobbering an in-flight edit.
    onMutate: async (req) => {
      await qc.cancelQueries({ queryKey: keys.aiGenerations.all });
      const previous = qc.getQueryData<AiGenerationRecord[]>(keys.aiGenerations.all);
      qc.setQueryData<AiGenerationRecord[]>(keys.aiGenerations.all, (old) =>
        (old ?? []).map((g) =>
          g.id === req.id
            ? {
                ...g,
                ...(req.resumeText !== undefined ? { resumeText: req.resumeText } : {}),
                ...(req.coverLetterText !== undefined
                  ? { coverLetterText: req.coverLetterText }
                  : {}),
              }
            : g
        )
      );
      return { previous };
    },
    onError: (_err, _req, ctx) => {
      if (ctx?.previous) qc.setQueryData(keys.aiGenerations.all, ctx.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.aiGenerations.all }),
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

export const useRemoveAiGenerationsBulk = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => api.aiGenerations.removeBulk(ids),
    // Optimistic bulk delete: drop all selected cards immediately, restore on failure.
    onMutate: async (ids) => {
      await qc.cancelQueries({ queryKey: keys.aiGenerations.all });
      const previous = qc.getQueryData<AiGenerationRecord[]>(keys.aiGenerations.all);
      const idSet = new Set(ids);
      qc.setQueryData<AiGenerationRecord[]>(keys.aiGenerations.all, (old) =>
        (old ?? []).filter((g) => !idSet.has(g.id))
      );
      return { previous };
    },
    onError: (_err, _ids, ctx) => {
      if (ctx?.previous) qc.setQueryData(keys.aiGenerations.all, ctx.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: keys.aiGenerations.all }),
  });
};
