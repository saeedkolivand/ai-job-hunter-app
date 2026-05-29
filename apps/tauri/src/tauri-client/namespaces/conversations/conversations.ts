import { invoke } from '@tauri-apps/api/core';

import type { ConversationSaveMessageRequest } from '@ajh/shared/schemas';

export const conversations = {
  getOrCreateConversation: () => invoke('conversations_get_or_create'),
  loadMessages: ({ conversationId }: { conversationId: string }) =>
    invoke('conversations_load_messages', { conversationId }),
  saveMessage: (req: ConversationSaveMessageRequest) =>
    invoke('conversations_save_message', { req }),
  saveAllMessages: (opts: Record<string, unknown>) =>
    invoke('conversations_save_all_messages', opts),
};
