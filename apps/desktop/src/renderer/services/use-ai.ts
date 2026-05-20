import { useEffect, useRef } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';

import type { AiGenerateRequest, AiStreamChunk } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export const useAIModels = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.models,
    queryFn: () => api.ai.listModels(),
    staleTime: 60_000,
  });
};

export const useGenerateAI = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (req: AiGenerateRequest) => api.ai.generate(req) });
};

export const usePullModel = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (model: string) => api.ai.pullModel(model) });
};

export const useUnloadModel = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (model: string) => api.ai.unloadModel(model) });
};

export const useAIStream = (onChunk: (chunk: AiStreamChunk) => void) => {
  const api = useAppClient();
  const onChunkRef = useRef(onChunk);
  onChunkRef.current = onChunk;

  useEffect(() => {
    const offRaw = api.ai.onStream((chunk: unknown) => onChunkRef.current(chunk as AiStreamChunk));
    const off = offRaw as unknown as (() => void) | undefined;
    return () => off?.();
  }, [api]);
};
