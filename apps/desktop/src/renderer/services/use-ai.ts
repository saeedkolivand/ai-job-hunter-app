import { useEffect, useRef } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';

import type { AiGenerateRequest, AiStreamChunk } from '@ajh/shared';

import { keys } from './query-client';

export const useAIModels = () =>
  useQuery({
    queryKey: keys.ai.models,
    queryFn: () => window.api.ai.listModels(),
    staleTime: 60_000,
  });

export const useGenerateAI = () =>
  useMutation({
    mutationFn: (req: AiGenerateRequest) => window.api.ai.generate(req),
  });

export const usePullModel = () =>
  useMutation({
    mutationFn: (model: string) => window.api.ai.pullModel(model),
  });

export const useUnloadModel = () =>
  useMutation({
    mutationFn: (model: string) => window.api.ai.unloadModel(model),
  });

/**
 * Subscribe to the AI token stream.
 * `onChunk` is called with each incoming token.
 * Returns a cleanup function automatically via useEffect.
 */
export const useAIStream = (onChunk: (chunk: AiStreamChunk) => void) => {
  const onChunkRef = useRef(onChunk);
  onChunkRef.current = onChunk; // always call the latest version

  useEffect(() => {
    const offRaw = window.api?.ai.onStream((chunk: unknown) =>
      onChunkRef.current(chunk as AiStreamChunk)
    );
    const off = offRaw as unknown as (() => void) | undefined;
    return () => off?.();
  }, []); // empty deps — subscription is stable
};
