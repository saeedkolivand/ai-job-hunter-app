import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function ai(opts: WebHttpClientOptions) {
  const { cmd, subscribe } = createHttpClientHelpers(opts);
  return {
    generate: (req: unknown) => cmd('ai', 'generate', req),
    listModels: () => cmd('ai', 'listModels'),
    pullModel: (model: string) => cmd('ai', 'pullModel', { model }),
    unloadModel: (model: string) => cmd('ai', 'unloadModel', { model }),
    embed: (req: unknown) => cmd('ai', 'embed', req),
    onStream: (handler: (chunk: unknown) => void) => subscribe('ai.stream', handler),
    setProviderKey: (req: unknown) => cmd('ai', 'setProviderKey', req),
    removeProviderKey: (req: unknown) => cmd('ai', 'removeProviderKey', req),
    hasProviderKey: (req: unknown) => cmd('ai', 'hasProviderKey', req),
    listProviderModels: (req: unknown) => cmd('ai', 'listProviderModels', req),
  };
}
