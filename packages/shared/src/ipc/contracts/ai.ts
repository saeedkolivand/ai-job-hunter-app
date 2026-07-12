import type { AiGenerateRequest, ModelInspectResult } from '../../schemas/index.js';
import type { AiStreamChunk } from '../../types/index.js';

export interface AiContract {
  generate(req: AiGenerateRequest): Promise<{ jobId: string }>;

  /**
   * Stream a generation through the backend orchestration pipeline. Same wire
   * shape as `generate`, but the work runs as a composable `Pipeline` (so feature
   * generators share one lifecycle). Used by resume/cover-letter generation.
   */
  generatePipeline(req: AiGenerateRequest): Promise<{ jobId: string }>;

  onStream(handler: (chunk: AiStreamChunk) => void): () => void;

  listModels(): Promise<Array<{ name: string }>>;

  /**
   * Inspect a local (Ollama) model's real context window + size via `/api/show`,
   * to suggest safe generation limits. Returns `null` for non-local providers or
   * an unreachable Ollama server.
   */
  inspectModel(req: { model: string }): Promise<ModelInspectResult | null>;

  /**
   * Research the company named in a job ad and return a short factual brief —
   * used by the cover-letter "fit" paragraph and company-specific application
   * answers. Reuses the shared enricher: the active provider's own web search
   * (native tool, or the Ollama Web Search API for Ollama), cached. Degrades
   * gracefully — an empty brief, never an error, when the provider can't search
   * or the search fails. The brief is reference context only; the prompt treats
   * it as untrusted.
   */
  researchCompany(req: {
    jobAd: string;
    /** Accurate AI-extracted company name; preferred over heuristic job-ad extraction. */
    company?: string;
    provider?: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ company: string; brief: string }>;

  /**
   * Web-grounded market salary-range lookup for the salary application
   * question. Reuses the active provider's own web search (same channel as
   * `researchCompany`), parsed and strictly validated server-side, cached.
   * Degrades gracefully — `null`, never an error, when the provider can't
   * search, the search yields nothing reliable, or times out. Only validated
   * integers + a sanitized currency code are ever returned; no raw web text
   * crosses this boundary.
   */
  lookupSalary(req: {
    role: string;
    company?: string;
    location?: string;
    /** ISO-3166 alpha-2 job country, when known — grounds `currency` below. */
    country?: string;
    /** Authoritative ISO-4217 currency for `country` (resolve client-side via
     *  `countryToCurrency`). Pins the researched/reported currency server-side
     *  so a blank/weak `location` can't let the model default to USD or
     *  hallucinate one; omitted when the country is unknown, which preserves
     *  today's unconstrained "local currency for that location" behavior. */
    currency?: string;
    provider?: string;
    model?: string;
    baseUrl?: string;
  }): Promise<SalaryRange | null>;

  /**
   * Best-effort, per-question web-search reference notes for an application
   * answer — opt-in sibling of `researchCompany`, scoped to a single
   * question's topic (combines it with the role + company for relevance)
   * rather than a general company brief. Reuses the same backend enricher
   * channel: the active provider's own web search (native tool, or the Ollama
   * Web Search API), gated on the provider's actual search support. Degrades
   * gracefully — an empty string, never an error, when the provider can't
   * search or the search fails, so answer generation always proceeds exactly
   * as without web search. The notes are reference context only; the prompt
   * layer fences them as untrusted and never lets them write the answer.
   */
  researchAnswer(req: {
    question: string;
    role?: string;
    company?: string;
    provider?: string;
    model?: string;
    baseUrl?: string;
  }): Promise<string>;

  pullModel(model: string): Promise<{ jobId: string }>;

  unloadModel(model: string): Promise<void>;

  /** Synchronous embedding — returns the vector. Falls back gracefully if Ollama is offline. */
  embed(req: { text: string; model?: string }): Promise<{ vector: number[]; dim: number } | null>;

  /** Store an API key for a cloud AI provider in the OS keychain. */
  setProviderKey(req: { provider: string; apiKey: string }): Promise<{ success: boolean }>;

  /** Remove a stored provider API key from the OS keychain. */
  removeProviderKey(req: { provider: string }): Promise<{ success: boolean }>;

  /** Check whether a provider API key is stored (does not return the key). */
  hasProviderKey(req: { provider: string }): Promise<{ has: boolean }>;

  /**
   * Test whether a stored provider API key is valid by making a lightweight API call.
   * `baseUrl` is forwarded for OpenAI-compatible servers (LM Studio, vLLM, etc.).
   */
  testProviderKey(req: {
    provider: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string }>;

  /**
   * Fetch available models from a cloud provider using its stored API key.
   * `baseUrl` is forwarded for OpenAI-compatible servers.
   */
  listProviderModels(req: { provider: string; baseUrl?: string }): Promise<Array<{ name: string }>>;

  /**
   * Static, network-free capability probe for a provider/model — currently just
   * whether it can attempt a web-grounded company/role search. Reads the Rust
   * `ModelCapabilities` matrix (the same value the backend gates `research*` on),
   * so the renderer never mirrors the per-provider booleans and a new provider
   * needs zero TS change. Drives the capability-driven default of the tailoring
   * "search company" toggle. Unknown/unresolvable providers degrade to
   * `{ supportsWebSearch: false }`. `baseUrl` is forwarded for OpenAI-compatible
   * servers.
   */
  modelCapabilities(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ supportsWebSearch: boolean }>;

  /** Active embedding space, per-space vector counts, and document index coverage. */
  embeddingStatus(): Promise<EmbeddingStatus>;

  /**
   * Set the active embedding provider/model. An empty model resolves to the
   * provider's default. Changing it changes the embedding space — call
   * `reembedAll` afterwards to rebuild the index.
   */
  setEmbeddingConfig(req: {
    provider: string;
    model?: string;
    baseUrl?: string;
  }): Promise<{ success: boolean; error?: string; config?: EmbeddingConfig }>;

  /** Re-embed every document with the active embedding config. Returns a job id. */
  reembedAll(): Promise<{ jobId: string }>;

  /**
   * Read-only AI-spend summary: today's REAL per-provider token totals — as
   * reported by each provider's own response, never estimated — plus an
   * ESTIMATED USD cost from a static list-price rate table. The dollar
   * figure is a best-effort ballpark (BYO-key users have no billing API to
   * query), not a billing-accurate source. Local (Ollama) and CLI-agent
   * calls always cost $0.
   */
  spendSummary(): Promise<AiSpendSummary>;
}

export interface EmbeddingConfig {
  provider: string;
  model: string;
  baseUrl?: string | null;
}

/** A validated web-researched market salary range (mirrors the Rust
 *  `salary_research::SalaryRange` — min/max/currency only, already validated
 *  server-side before it crosses the IPC boundary). */
export interface SalaryRange {
  min: number;
  max: number;
  currency: string;
}

export interface EmbeddingSpaceInfo {
  provider: string;
  model: string;
  dim: number;
  count: number;
  active: boolean;
}

export interface EmbeddingStatus {
  active: EmbeddingConfig;
  spaces: EmbeddingSpaceInfo[];
  documents: { total: number; indexedInActiveSpace: number; stale: number };
}

/** One provider's real token totals + estimated cost, since the start of the
 *  current UTC day. */
export interface AiSpendProviderTotals {
  provider: string;
  inputTokens: number;
  outputTokens: number;
  estCostUsd: number;
}

/** Today's real AI-spend totals, overall and per provider. */
export interface AiSpendSummary {
  today: { inputTokens: number; outputTokens: number; estCostUsd: number };
  perProvider: AiSpendProviderTotals[];
}

export const AI_CHANNELS = {
  generate: 'ai:generate',
  listModels: 'ai:listModels',
  pullModel: 'ai:pullModel',
  unloadModel: 'ai:unloadModel',
  embed: 'ai:embed',
} as const;
