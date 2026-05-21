import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export const useConversation = (conversationId: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.conversations.detail(conversationId),
    queryFn: () => api.conversations.loadMessages(conversationId),
    enabled: !!conversationId,
  });
};

export const useGetOrCreateConversation = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: () => api.conversations.getOrCreateConversation() as Promise<{ id: string }>,
  });
};

export const useSaveMessage = (conversationId: string) => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (msg: { role: 'user' | 'assistant' | 'system'; content: string }) =>
      api.conversations.saveMessage({ conversationId, ...msg }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.conversations.detail(conversationId) }),
  });
};
