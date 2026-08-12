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
  // `contextWindow` is REPLACE-semantics like model/baseUrl — send the window
  // held for the model being saved, or the backend stores none.
  setProviderSettings: ({
    provider,
    model,
    baseUrl,
    contextWindow,
  }: {
    provider: string;
    model?: string;
    baseUrl?: string;
    contextWindow?: number;
  }) => invoke('ai_set_provider_settings', { provider, model, baseUrl, contextWindow }),
  seedActiveConfig: ({
    config,
  }: {
    config: {
      activeProvider?: string;
      providers: Record<string, { model?: string; baseUrl?: string; contextWindow?: number }>;
      stageOverrides?: Record<
        string,
        { provider: string; model: string; baseUrl?: string; contextWindow?: number }
      >;
    };
  }) => invoke('ai_seed_active_config', { config }),
  // Per-stage model overrides. An absent key means "the active provider" —
  // never write one back just to mirror the default.
  stageOverrides: () => invoke('ai_stage_overrides'),
  setStageOverride: ({
    stage,
    provider,
    model,
    baseUrl,
    contextWindow,
  }: {
    stage: string;
    provider: string;
    model?: string;
    baseUrl?: string;
    contextWindow?: number;
  }) => invoke('ai_set_stage_override', { stage, provider, model, baseUrl, contextWindow }),
  clearStageOverride: ({ stage }: { stage: string }) =>
    invoke('ai_clear_stage_override', { stage }),
  // `effort` on both research calls: the backend's deadline around
  // search + synthesis scales with it, the same way the generation stream
  // deadline does. Omitting it leaves the unscaled baseline.
  researchCompany: ({
    jobAd,
    company,
    role,
    effort,
  }: {
    jobAd: string;
    company?: string;
    role?: string;
    effort?: string;
  }) => invoke('ai_research_company', { jobAd, company, role, effort }),
  lookupSalary: ({
    role,
    company,
    location,
    country,
    currency,
    effort,
  }: {
    role: string;
    company?: string;
    location?: string;
    country?: string;
    currency?: string;
    effort?: string;
  }) =>
    invoke('ai_lookup_salary', {
      role,
      company,
      location,
      country,
      currency,
      effort,
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
  indexStaleDocuments: () => invoke('ai_index_stale_documents'),
  spendSummary: () => invoke('ai_spend_summary'),
};
