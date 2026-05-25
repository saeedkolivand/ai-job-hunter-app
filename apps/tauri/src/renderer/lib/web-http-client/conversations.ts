import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function conversations(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    getOrCreateConversation: () => cmd('conversations', 'getOrCreateConversation'),
    loadMessages: ({ conversationId }: { conversationId: string }) =>
      cmd('conversations', 'loadMessages', { conversationId }),
    saveMessage: (req: unknown) => cmd('conversations', 'saveMessage', req),
    saveAllMessages: (opts: Record<string, unknown>) =>
      cmd('conversations', 'saveAllMessages', opts),
  };
}
