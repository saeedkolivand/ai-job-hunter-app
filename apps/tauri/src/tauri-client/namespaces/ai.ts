import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { AiGenerateRequest, EmbedRequest } from '@ajh/shared/schemas';
import type { AiStreamChunk } from '@ajh/shared/types';

import { asyncUnsub } from '../utils.js';

export const ai = {
  generate: (req: AiGenerateRequest) => invoke('ai_generate', { req }),
  listModels: () => invoke('ai_list_models'),
  pullModel: (model: string) => invoke('ai_pull_model', { model }),
  unloadModel: (model: string) => invoke('ai_unload_model', { model }),
  embed: (req: EmbedRequest) => invoke('ai_embed', { req }),
  onStream: (handler: (chunk: AiStreamChunk) => void) =>
    asyncUnsub(() => listen<AiStreamChunk>('ai:stream', (e) => handler(e.payload))),
  setProviderKey: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
    invoke('ai_set_provider_key', { provider, apiKey }),
  removeProviderKey: ({ provider }: { provider: string }) =>
    invoke('ai_remove_provider_key', { provider }),
  hasProviderKey: ({ provider }: { provider: string }) =>
    invoke('ai_has_provider_key', { provider }),
  testProviderKey: ({ provider }: { provider: string }) =>
    invoke('ai_test_provider_key', { provider }),
  listProviderModels: ({ provider }: { provider: string }) =>
    invoke('ai_list_provider_models', { provider }),
};
