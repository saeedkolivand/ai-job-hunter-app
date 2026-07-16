import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { EVENT_CHANNELS } from '@ajh/shared';
import type { AiGenerateRequest, EmbedRequest } from '@ajh/shared/schemas';
import type { AiStreamChunk } from '@ajh/shared/types';

import { asyncUnsub } from '../../utils.js';

export const ai = {
  generate: (req: AiGenerateRequest) => invoke('ai_generate', { req }),
  generatePipeline: (req: AiGenerateRequest) => invoke('generate_pipeline', { req }),
  listModels: () => invoke('ai_list_models'),
  inspectModel: ({ model }: { model: string }) => invoke('ai_inspect_model', { model }),
  // Backend-owned active generation provider config (task #16). Routing comes
  // from the store here, never the request — closes the base_url SSRF.
  activeConfig: () => invoke('ai_active_config'),
  setActiveProvider: ({ provider }: { provider: string }) =>
    invoke('ai_set_active_provider', { provider }),
  setProviderSettings: ({
    provider,
    model,
    baseUrl,
  }: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }) => invoke('ai_set_provider_settings', { provider, model, baseUrl }),
  seedActiveConfig: ({
    config,
  }: {
    config: {
      activeProvider?: string;
      providers: Record<string, { model?: string; baseUrl?: string }>;
    };
  }) => invoke('ai_seed_active_config', { config }),
  researchCompany: ({ jobAd, company }: { jobAd: string; company?: string }) =>
    invoke('ai_research_company', { jobAd, company }),
  lookupSalary: ({
    role,
    company,
    location,
    country,
    currency,
  }: {
    role: string;
    company?: string;
    location?: string;
    country?: string;
    currency?: string;
  }) =>
    invoke('ai_lookup_salary', {
      role,
      company,
      location,
      country,
      currency,
    }),
  researchAnswer: ({
    question,
    role,
    company,
  }: {
    question: string;
    role?: string;
    company?: string;
  }) => invoke('ai_research_answer', { question, role, company }),
  pullModel: (model: string) => invoke('ai_pull_model', { model }),
  unloadModel: (model: string) => invoke('ai_unload_model', { model }),
  embed: (req: EmbedRequest) => invoke('ai_embed', { req }),
  onStream: (handler: (chunk: AiStreamChunk) => void) =>
    asyncUnsub(() => listen<AiStreamChunk>(EVENT_CHANNELS.ai.stream, (e) => handler(e.payload))),
  setProviderKey: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
    invoke('ai_set_provider_key', { provider, apiKey }),
  removeProviderKey: ({ provider }: { provider: string }) =>
    invoke('ai_remove_provider_key', { provider }),
  hasProviderKey: ({ provider }: { provider: string }) =>
    invoke('ai_has_provider_key', { provider }),
  testProviderKey: ({ provider, baseUrl }: { provider: string; baseUrl?: string }) =>
    invoke('ai_test_provider_key', { provider, baseUrl }),
  listProviderModels: ({ provider, baseUrl }: { provider: string; baseUrl?: string }) =>
    invoke('ai_list_provider_models', { provider, baseUrl }),
  modelCapabilities: ({
    provider,
    model,
    baseUrl,
  }: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }) => invoke('ai_model_capabilities', { provider, model, baseUrl }),
  embeddingStatus: () => invoke('ai_embedding_status'),
  setEmbeddingConfig: ({
    provider,
    model,
    baseUrl,
  }: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }) => invoke('ai_set_embedding_config', { provider, model, baseUrl }),
  reembedAll: () => invoke('ai_reembed_all'),
  spendSummary: () => invoke('ai_spend_summary'),
};
