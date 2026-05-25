export interface ConversationsContract {
  /** Get or create the current conversation */
  getOrCreateConversation(): Promise<{ id: string; title: string }>;

  /** Load messages for a conversation */
  loadMessages(opts: { conversationId: string }): Promise<
    Array<{
      id: string;
      role: 'user' | 'assistant' | 'system';
      content: string;
      createdAt: number;
    }>
  >;

  /** Save a single message */
  saveMessage(opts: {
    conversationId: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
  }): Promise<{ success: boolean }>;

  /** Save all messages to database */
  saveAllMessages(opts: {
    conversationId: string;
    messages: Array<{
      id: string;
      role: 'user' | 'assistant' | 'system';
      content: string;
      createdAt: number;
    }>;
  }): Promise<{ success: boolean }>;
}

export const CONVERSATIONS_CHANNELS = {
  getOrCreateConversation: 'conversations:getOrCreateConversation',
  loadMessages: 'conversations:loadMessages',
  saveMessage: 'conversations:saveMessage',
} as const;
