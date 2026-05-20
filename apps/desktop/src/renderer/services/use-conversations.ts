import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { keys } from './query-client';

export const useConversation = (conversationId: string) =>
  useQuery({
    queryKey: keys.conversations.detail(conversationId),
    // preload signature: loadMessages(conversationId: string)
    queryFn: () => window.api.conversations.loadMessages(conversationId),
    enabled: !!conversationId,
  });

export const useGetOrCreateConversation = () =>
  useMutation({
    mutationFn: () => window.api.conversations.getOrCreateConversation() as Promise<{ id: string }>,
  });

export const useSaveMessage = (conversationId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (msg: { role: 'user' | 'assistant' | 'system'; content: string }) =>
      window.api.conversations.saveMessage({ conversationId, ...msg }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.conversations.detail(conversationId) }),
  });
};
