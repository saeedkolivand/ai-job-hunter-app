import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';
import { useAiProviderConfig } from '@/store/preferences-store';

import { keys } from './query-client';

export const useHasProviderKey = (provider: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.models, 'provider-key', provider],
    queryFn: () => api.ai.hasProviderKey({ provider }),
    enabled: provider !== 'ollama',
    staleTime: 30_000,
  });
};

export const useSetProviderKey = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
      api.ai.setProviderKey({ provider, apiKey }),
    onSuccess: (_data, { provider }) => {
      qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-key', provider] });
      qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-models', provider] });
    },
  });
};

export const useRemoveProviderKey = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ provider }: { provider: string }) => api.ai.removeProviderKey({ provider }),
    onSuccess: (_data, { provider }) => {
      qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-key', provider] });
    },
  });
};

export const useListProviderModels = (provider: string, enabled = true) => {
  const api = useAppClient();
  return useQuery({
    queryKey: [...keys.ai.models, 'provider-models', provider],
    queryFn: () => api.ai.listProviderModels({ provider }),
    enabled: enabled && provider !== 'ollama',
    staleTime: 300_000,
  });
};

/** Returns the provider/model/baseUrl to inject into every ai_generate call. */
export const useGenerateConfig = () => {
  const config = useAiProviderConfig();
  return {
    provider: config?.provider ?? 'ollama',
    model: config?.model ?? '',
    baseUrl: config?.baseUrl,
  };
};
